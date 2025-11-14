//! Core CLI loop, dispatch, and shell context helpers.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    env, io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, Utc};
use dialoguer::{theme::ColorfulTheme, Input, Select};
use strsim::levenshtein;
use uuid::Uuid;

use crate::{
    config::{Config, ConfigManager},
    core::errors::BudgetError,
    core::ledger_manager::LedgerManager,
    core::services::{
        AccountService, CategoryBudgetStatus, CategoryBudgetSummary, CategoryService, ServiceError,
        SummaryService, TransactionService,
    },
    currency::{format_currency_value, format_currency_value_with_precision, format_date},
    domain::BudgetPeriod as CategoryBudgetPeriod,
    ledger::{
        account::AccountKind, category::CategoryKind, Account, BudgetPeriod, BudgetScope,
        BudgetStatus, BudgetSummary, Category, DateWindow, ForecastReport, Ledger, Recurrence,
        RecurrenceEnd, RecurrenceMode, RecurrenceSnapshot, RecurrenceStatus, ScheduledStatus,
        SimulationBudgetImpact, SimulationChange, SimulationTransactionPatch, TimeInterval,
        TimeUnit, Transaction, TransactionStatus,
    },
    storage::json_backend::JsonStorage,
};

use crate::cli::forms::{
    AccountFormData, AccountInitialData, AccountWizard, CategoryFormData, CategoryInitialData,
    CategoryWizard, FormEngine, FormResult, TransactionFormData, TransactionInitialData,
    TransactionRecurrenceAction, TransactionWizard, WizardInteraction,
};
use crate::cli::selection::{
    providers::{
        AccountSelectionProvider, CategorySelectionProvider, ConfigBackupSelectionProvider,
        LedgerBackupSelectionProvider, ProviderError, SimulationSelectionProvider,
        TransactionSelectionProvider,
    },
    SelectionError, SelectionManager,
};
use crate::cli::selectors::{SelectionOutcome, SelectionProvider};
pub use crate::core::errors::CliError;

use super::commands;
use super::io as cli_io;
use super::output::render_table as output_table;
use super::registry::{CommandEntry, CommandRegistry};
use crate::cli::shell_context::SelectionOverride;
pub use crate::cli::shell_context::{CliMode, ShellContext};
use crate::cli::ui::banner::Banner;
use crate::cli::ui::formatting::Formatter;
use crate::cli::ui::prompts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoopControl {
    Continue,
    Exit,
}

pub type CommandResult = Result<(), CommandError>;

impl ShellContext {
    fn auto_load_last(&mut self) -> Result<(), CliError> {
        if self.mode != CliMode::Interactive {
            return Ok(());
        }
        if self.manager().with_current(|_| ()).is_ok() {
            return Ok(());
        }
        let last_opened = { self.config_read().last_opened_ledger.clone() };
        let Some(name) = last_opened else {
            return Ok(());
        };
        let load_result = {
            let mut manager = self.manager_mut();
            manager.load(&name)
        };
        if let Ok(report) = load_result {
            let path = self.storage.ledger_path(&name);
            self.ledger_path = Some(path);
            self.clear_active_simulation();
            self.report_load(&report.warnings, &report.migrations);
            cli_io::print_success(format!("Automatically loaded last ledger `{}`.", name));
        }
        Ok(())
    }

    pub(crate) fn manager(&self) -> RwLockReadGuard<'_, LedgerManager> {
        self.ledger_manager
            .read()
            .expect("LedgerManager lock poisoned")
    }

    pub(crate) fn manager_mut(&self) -> RwLockWriteGuard<'_, LedgerManager> {
        self.ledger_manager
            .write()
            .expect("LedgerManager lock poisoned")
    }

    fn config_read(&self) -> RwLockReadGuard<'_, Config> {
        self.config.read().expect("Config lock poisoned")
    }

    pub(crate) fn config_write(&self) -> RwLockWriteGuard<'_, Config> {
        self.config.write().expect("Config lock poisoned")
    }

    fn config_manager(&self) -> RwLockReadGuard<'_, ConfigManager> {
        self.config_manager
            .read()
            .expect("ConfigManager lock poisoned")
    }

    pub(crate) fn persist_config(&self) -> Result<(), CommandError> {
        let config = self.config_read();
        self.config_manager()
            .save(&config)
            .map_err(CommandError::from_core)
    }

    fn update_last_opened(&mut self, name: Option<&str>) -> CommandResult {
        {
            let mut config = self.config_write();
            config.last_opened_ledger = name.map(|value| value.to_string());
        }
        self.persist_config()
    }

    pub fn new(mode: CliMode) -> Result<Self, CliError> {
        let mut registry = CommandRegistry::new();
        commands::register_all(&mut registry);

        let storage = JsonStorage::new_default().map_err(CliError::from)?;
        let manager = Arc::new(RwLock::new(LedgerManager::new(Box::new(storage.clone()))));
        let config_manager_raw = ConfigManager::new().map_err(CliError::from)?;
        let config = config_manager_raw.load().map_err(CliError::from)?;
        cli_io::apply_config(&config);
        let config = Arc::new(RwLock::new(config));
        let config_manager = Arc::new(RwLock::new(config_manager_raw));

        let mut app = ShellContext {
            mode,
            registry,
            ledger_manager: manager,
            theme: ColorfulTheme::default(),
            storage,
            config_manager,
            config,
            ledger_path: None,
            active_simulation_name: None,
            selection_override: None,
            current_simulation: None,
            last_command: None,
            running: true,
        };

        if let Some(selection_override) = selection_override_from_env() {
            app.selection_override = Some(selection_override);
        }

        app.auto_load_last()?;
        Ok(app)
    }

    pub(crate) fn mode(&self) -> CliMode {
        self.mode
    }

    #[allow(dead_code)]
    pub(crate) fn theme(&self) -> &ColorfulTheme {
        &self.theme
    }

    pub(crate) fn ledger_name(&self) -> Option<String> {
        self.manager().current_name().map(|name| name.to_string())
    }

    pub(crate) fn ledger_path(&self) -> Option<PathBuf> {
        self.ledger_path.clone()
    }

    pub(crate) fn set_active_simulation(&mut self, name: Option<String>) {
        match name {
            Some(sim_name) => {
                let simulation = self
                    .with_ledger(|ledger| Ok(ledger.simulation(&sim_name).cloned()))
                    .ok()
                    .flatten();
                self.current_simulation = simulation;
                self.active_simulation_name = Some(sim_name);
            }
            None => {
                self.current_simulation = None;
                self.active_simulation_name = None;
            }
        }
    }

    pub(crate) fn clear_active_simulation(&mut self) {
        self.current_simulation = None;
        self.active_simulation_name = None;
    }

    fn ensure_base_mode(&self, action: &str) -> Result<(), CommandError> {
        if self.is_simulation_active() {
            Err(CommandError::InvalidArguments(format!(
                "{} is unavailable while editing a simulation. Use `leave-simulation` first.",
                action
            )))
        } else {
            Ok(())
        }
    }

    fn format_amount(&self, ledger: &Ledger, amount: f64) -> String {
        let precision_override = {
            let config = self.config_read();
            config.default_currency_precision
        };
        format_currency_value_with_precision(
            amount,
            ledger.base_currency(),
            &ledger.locale,
            &ledger.format,
            precision_override,
        )
    }

    fn format_date(&self, ledger: &Ledger, date: NaiveDate) -> String {
        format_date(&ledger.locale, date)
    }

    fn config_default_category_period(&self) -> CategoryBudgetPeriod {
        let raw_value = {
            let config = self.config_read();
            config.default_budget_period.clone()
        };
        parse_category_budget_period_str(&raw_value).unwrap_or(CategoryBudgetPeriod::Monthly)
    }

    fn describe_budget_period_label(
        &self,
        ledger: &Ledger,
        period: &CategoryBudgetPeriod,
        reference_date: Option<NaiveDate>,
    ) -> String {
        let mut label = describe_category_budget_period(period);
        if let Some(date) = reference_date {
            label.push_str(&format!(" • anchor {}", self.format_date(ledger, date)));
        }
        label
    }

    fn category_budget_row(&self, ledger: &Ledger, status: &CategoryBudgetStatus) -> Vec<String> {
        let budget = status
            .budget
            .as_ref()
            .expect("row rendering requires budget details");
        vec![
            status.name.clone(),
            self.format_amount(ledger, budget.amount),
            self.format_amount(ledger, status.totals.real),
            self.format_amount(ledger, status.totals.remaining),
            self.describe_budget_period_label(ledger, &budget.period, budget.reference_date),
            format!("{:?}", status.totals.status),
        ]
    }

    pub(crate) fn show_config(&self) -> CommandResult {
        let config = self.config_read();
        Formatter::new().print_header("Configuration");
        cli_io::print_info(format!("  Locale: {}", config.locale));
        cli_io::print_info(format!("  Currency: {}", config.currency));
        cli_io::print_info(format!(
            "  Theme: {}",
            config.theme.as_deref().unwrap_or("default")
        ));
        cli_io::print_info(format!(
            "  Last opened ledger: {}",
            config.last_opened_ledger.as_deref().unwrap_or("(none)")
        ));
        cli_io::print_info(format!(
            "  Default budget period: {}",
            config.default_budget_period
        ));
        cli_io::print_info(format!(
            "  Currency precision override: {}",
            config
                .default_currency_precision
                .map(|value| format!("{value} places"))
                .unwrap_or_else(|| "auto".into())
        ));
        let _ = self.with_ledger(|ledger| {
            Formatter::new().print_header("Ledger Format");
            cli_io::print_info(format!(
                "  Base currency: {}",
                ledger.base_currency.as_str()
            ));
            cli_io::print_info(format!("  Locale: {}", ledger.locale.language_tag));
            cli_io::print_info(format!(
                "  Negative style: {:?}",
                ledger.format.negative_style
            ));
            cli_io::print_info(format!(
                "  Screen reader mode: {}",
                if ledger.format.screen_reader_mode {
                    "on"
                } else {
                    "off"
                }
            ));
            cli_io::print_info(format!(
                "  High contrast mode: {}",
                if ledger.format.high_contrast_mode {
                    "on"
                } else {
                    "off"
                }
            ));
            cli_io::print_info(format!("  Valuation policy: {:?}", ledger.valuation_policy));
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn set_config_value(&mut self, key: &str, value: &str) -> CommandResult {
        {
            let mut config = self.config_write();
            match key.to_lowercase().as_str() {
                "locale" => config.locale = value.to_string(),
                "currency" => config.currency = value.to_string(),
                "theme" => {
                    if value.eq_ignore_ascii_case("none") || value.is_empty() {
                        config.theme = None;
                    } else {
                        config.theme = Some(value.to_string());
                    }
                }
                "last_opened_ledger" => {
                    if value.eq_ignore_ascii_case("none") || value.is_empty() {
                        config.last_opened_ledger = None;
                    } else {
                        config.last_opened_ledger = Some(value.to_string());
                    }
                }
                "default_budget_period" => {
                    let period = parse_category_budget_period_str(value)?;
                    config.default_budget_period = category_budget_period_token(&period);
                }
                "default_currency_precision" => {
                    if value.eq_ignore_ascii_case("auto") || value.is_empty() {
                        config.default_currency_precision = None;
                    } else {
                        let parsed: u8 = value.parse().map_err(|_| {
                            CommandError::InvalidArguments(
                                "default_currency_precision must be numeric (0-6)".into(),
                            )
                        })?;
                        if parsed > 6 {
                            return Err(CommandError::InvalidArguments(
                                "default_currency_precision must be between 0 and 6".into(),
                            ));
                        }
                        config.default_currency_precision = Some(parsed);
                    }
                }
                other => {
                    return Err(CommandError::InvalidArguments(format!(
                        "unknown config key `{}`",
                        other
                    )))
                }
            }
        }
        self.persist_config()?;
        let config = self.config_read();
        cli_io::apply_config(&config);
        cli_io::print_success("Configuration updated.");
        Ok(())
    }

    pub(crate) fn require_named_ledger(&self) -> Result<String, CommandError> {
        let manager = self.manager();
        manager
            .current_name()
            .map(|name| name.to_string())
            .ok_or_else(|| {
                CommandError::InvalidArguments(
                    "No named ledger associated. Use `save-ledger <name>` once to bind it.".into(),
                )
            })
    }

    fn report_load(&self, warnings: &[String], migrations: &[String]) {
        for note in migrations {
            cli_io::print_info(format!("Migration: {}", note));
        }
        for warning in warnings {
            cli_io::print_warning(warning);
        }
    }

    pub(crate) fn dispatch(
        &mut self,
        command: &str,
        raw: &str,
        args: &[&str],
    ) -> Result<LoopControl, CommandError> {
        if let Some(handler) = self.registry.handler(command) {
            match handler(self, args) {
                Ok(()) => Ok(LoopControl::Continue),
                Err(CommandError::ExitRequested) => Ok(LoopControl::Exit),
                Err(err) => Err(err),
            }
        } else {
            self.suggest_command(raw);
            Ok(LoopControl::Continue)
        }
    }

    #[cfg(test)]
    pub(crate) fn process_line(&mut self, line: &str) -> Result<LoopControl, CommandError> {
        let tokens = match crate::cli::shell::parse_command_line(line) {
            Ok(tokens) => tokens,
            Err(err) => {
                self.print_warning(&err.to_string());
                return Ok(LoopControl::Continue);
            }
        };

        if tokens.is_empty() {
            return Ok(LoopControl::Continue);
        }

        let command = tokens[0].to_lowercase();
        let args: Vec<&str> = tokens.iter().skip(1).map(String::as_str).collect();
        self.dispatch(&command, &tokens[0], &args)
    }

    pub(crate) fn suggest_command(&self, input: &str) {
        cli_io::print_warning(format!(
            "Unknown command `{}`. Type `help` to see available commands.",
            input
        ));

        let mut suggestions: Vec<_> = self
            .registry
            .names()
            .map(|key| (levenshtein(key, input), key))
            .collect();
        suggestions.sort_by_key(|(distance, _)| *distance);

        if let Some((distance, best)) = suggestions.first() {
            if *distance <= 3 {
                cli_io::print_info(format!("Suggestion: `{}`?", best));
            }
        }
    }

    pub(crate) fn confirm_exit(&self) -> Result<bool, CliError> {
        if self.mode == CliMode::Script {
            return Ok(true);
        }
        cli_io::confirm_action("Exit shell?")
    }

    pub(crate) fn report_error(&self, err: CommandError) -> Result<(), CliError> {
        match err {
            CommandError::ExitRequested => Ok(()),
            CommandError::InvalidArguments(message) => {
                self.print_error(&message);
                self.print_hint("Use `help <command>` for usage details.");
                Ok(())
            }
            CommandError::LedgerNotLoaded => {
                self.print_error("Ledger not loaded. Use `ledger new` or `ledger load` first.");
                self.print_hint("Try `ledger new Demo monthly` to get started.");
                Ok(())
            }
            other => {
                self.print_error(&other.to_string());
                Ok(())
            }
        }
    }

    pub(crate) fn print_error(&self, message: &str) {
        cli_io::print_error(message);
    }

    pub(crate) fn print_warning(&self, message: &str) {
        cli_io::print_warning(message);
    }

    pub(crate) fn print_hint(&self, message: &str) {
        cli_io::print_hint(message);
    }

    pub(crate) fn await_menu_escape(&self) -> CommandResult {
        if self.mode != CliMode::Interactive {
            return Ok(());
        }
        Formatter::new().print_detail("Press ESC to return to the main menu.");
        prompts::wait_for_escape().map_err(CommandError::Io)
    }

    pub(crate) fn with_ledger<T>(
        &self,
        f: impl FnOnce(&Ledger) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let manager = self.manager();

        manager
            .with_current(|ledger| f(ledger))
            .map_err(CommandError::from_core)?
    }

    pub(crate) fn with_ledger_mut<T>(
        &self,
        f: impl FnOnce(&mut Ledger) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let manager = self.manager();

        manager
            .with_current_mut(|ledger| f(ledger))
            .map_err(CommandError::from_core)?
    }

    pub(crate) fn active_simulation_name(&self) -> Option<&str> {
        self.current_simulation
            .as_ref()
            .map(|sim| sim.name.as_str())
            .or(self.active_simulation_name.as_deref())
    }

    pub(crate) fn can_prompt(&self) -> bool {
        if self.mode == CliMode::Interactive {
            return true;
        }
        self.selection_override
            .as_ref()
            .is_some_and(|override_data| override_data.has_choices())
    }

    fn select_with<P>(
        &self,
        provider: P,
        prompt: &str,
        empty_message: &str,
    ) -> Result<Option<P::Id>, CommandError>
    where
        P: SelectionProvider,
        P::Id: Clone,
        CommandError: From<P::Error>,
    {
        let manager = SelectionManager::new(provider);
        let selection_choice = self
            .selection_override
            .as_ref()
            .and_then(|override_data| override_data.pop());
        let outcome = if let Some(choice) = selection_choice {
            manager.choose_with(prompt, empty_message, move |_, _| Ok(choice))
        } else {
            manager.choose_with_dialoguer(prompt, empty_message, &self.theme)
        };
        match outcome {
            Ok(SelectionOutcome::Selected(id)) => Ok(Some(id)),
            Ok(SelectionOutcome::Cancelled) => Ok(None),
            Err(SelectionError::Provider(err)) => Err(err.into()),
            Err(SelectionError::Interaction(err)) => Err(CommandError::Dialoguer(err)),
        }
    }

    pub(crate) fn select_transaction_index(
        &self,
        prompt: &str,
    ) -> Result<Option<usize>, CommandError> {
        self.select_with(
            TransactionSelectionProvider::new(self),
            prompt,
            "No transactions available.",
        )
    }

    fn select_simulation_name(&self, prompt: &str) -> Result<Option<String>, CommandError> {
        self.select_with(
            SimulationSelectionProvider::new(self),
            prompt,
            "No saved simulations available.",
        )
    }

    pub(crate) fn select_ledger_backup(
        &self,
        prompt: &str,
    ) -> Result<Option<String>, CommandError> {
        self.select_with(
            LedgerBackupSelectionProvider::new(self),
            prompt,
            "No backups available.",
        )
    }

    pub(crate) fn select_config_backup(
        &self,
        prompt: &str,
    ) -> Result<Option<String>, CommandError> {
        self.select_with(
            ConfigBackupSelectionProvider::new(self.config_manager.clone()),
            prompt,
            "No configuration backups found.",
        )
    }

    pub(crate) fn select_account_index(&self, prompt: &str) -> Result<Option<usize>, CommandError> {
        self.select_with(
            AccountSelectionProvider::new(self),
            prompt,
            "No accounts available.",
        )
    }

    pub(crate) fn select_category_index(
        &self,
        prompt: &str,
    ) -> Result<Option<usize>, CommandError> {
        self.select_with(
            CategorySelectionProvider::new(self),
            prompt,
            "No categories available.",
        )
    }

    fn resolve_category_target(
        &self,
        name_arg: Option<&str>,
        usage: &str,
        prompt: &str,
    ) -> Result<Option<(Uuid, String)>, CommandError> {
        if let Some(raw) = name_arg {
            let needle = raw.trim();
            if needle.is_empty() {
                return Err(CommandError::InvalidArguments(usage.into()));
            }
            return self
                .with_ledger(|ledger| {
                    ledger
                        .categories
                        .iter()
                        .find(|category| category.name.eq_ignore_ascii_case(needle))
                        .map(|category| Ok((category.id, category.name.clone())))
                        .unwrap_or_else(|| {
                            Err(CommandError::InvalidArguments(format!(
                                "category `{}` not found. Use `category list` to view available names.",
                                needle
                            )))
                        })
                })
                .map(Some);
        }
        if !self.can_prompt() {
            return Err(CommandError::InvalidArguments(usage.into()));
        }
        match self.select_category_index(prompt)? {
            Some(index) => self
                .with_ledger(|ledger| {
                    ledger
                        .categories
                        .get(index)
                        .map(|category| (category.id, category.name.clone()))
                        .ok_or_else(|| {
                            CommandError::InvalidArguments("category index out of range".into())
                        })
                })
                .map(Some),
            None => Ok(None),
        }
    }

    fn account_category_options(&self, ledger: &Ledger) -> Vec<(String, Option<Uuid>)> {
        ledger
            .categories
            .iter()
            .map(|category| {
                (
                    format!(
                        "{} ({:?}) [{}]",
                        category.name,
                        category.kind,
                        short_id(category.id)
                    ),
                    Some(category.id),
                )
            })
            .collect()
    }

    fn transaction_account_options(&self, ledger: &Ledger) -> Vec<(String, Uuid)> {
        ledger
            .accounts
            .iter()
            .map(|account| {
                (
                    format!(
                        "{} ({:?}) [{}]",
                        account.name,
                        account.kind,
                        short_id(account.id)
                    ),
                    account.id,
                )
            })
            .collect()
    }

    fn category_parent_options(
        &self,
        ledger: &Ledger,
        exclude: &HashSet<Uuid>,
    ) -> Vec<(String, Option<Uuid>)> {
        ledger
            .categories
            .iter()
            .filter(|category| !exclude.contains(&category.id))
            .map(|category| {
                (
                    format!(
                        "{} ({:?}) [{}]",
                        category.name,
                        category.kind,
                        short_id(category.id)
                    ),
                    Some(category.id),
                )
            })
            .collect()
    }

    fn category_descendants(&self, ledger: &Ledger, root: Uuid) -> HashSet<Uuid> {
        let mut descendants = HashSet::new();
        let mut stack = vec![root];
        while let Some(current) = stack.pop() {
            for category in ledger
                .categories
                .iter()
                .filter(|c| c.parent_id == Some(current))
            {
                if descendants.insert(category.id) {
                    stack.push(category.id);
                }
            }
        }
        descendants
    }

    fn apply_account_form(&mut self, data: AccountFormData) -> CommandResult {
        self.with_ledger_mut(|ledger| {
            match data.id {
                Some(id) => {
                    let mut changes = Account::new(data.name.clone(), data.kind);
                    changes.id = id;
                    changes.category_id = data.category_id;
                    changes.opening_balance = data.opening_balance;
                    changes.notes = data.notes.clone();
                    AccountService::edit(ledger, id, changes)?;
                    cli_io::print_success(format!("Account `{}` updated.", data.name));
                }
                None => {
                    let mut account = Account::new(data.name.clone(), data.kind);
                    account.category_id = data.category_id;
                    account.opening_balance = data.opening_balance;
                    account.notes = data.notes.clone();
                    AccountService::add(ledger, account)?;
                    cli_io::print_success(format!("Account `{}` added.", data.name));
                }
            }
            Ok(())
        })?;
        Ok(())
    }

    fn apply_category_form(&mut self, data: CategoryFormData) -> CommandResult {
        self.with_ledger_mut(|ledger| {
            match data.id {
                Some(id) => {
                    let mut changes = Category::new(data.name.clone(), data.kind);
                    changes.id = id;
                    changes.parent_id = data.parent_id;
                    changes.is_custom = data.is_custom;
                    changes.notes = data.notes.clone();
                    CategoryService::edit(ledger, id, changes)?;
                    cli_io::print_success(format!("Category `{}` updated.", data.name));
                }
                None => {
                    let mut category = Category::new(data.name.clone(), data.kind);
                    category.parent_id = data.parent_id;
                    category.is_custom = data.is_custom;
                    category.notes = data.notes.clone();
                    CategoryService::add(ledger, category)?;
                    cli_io::print_success(format!("Category `{}` added.", data.name));
                }
            }
            Ok(())
        })?;
        Ok(())
    }

    fn populate_transaction_from_form(transaction: &mut Transaction, data: &TransactionFormData) {
        transaction.from_account = data.from_account;
        transaction.to_account = data.to_account;
        transaction.category_id = data.category_id;
        transaction.scheduled_date = data.scheduled_date;
        transaction.budgeted_amount = data.budgeted_amount;

        let mut actual_date = data.actual_date;
        let mut actual_amount = data.actual_amount;

        if matches!(data.status, TransactionStatus::Completed) {
            if actual_date.is_none() {
                actual_date = Some(data.scheduled_date);
            }
            if actual_amount.is_none() {
                actual_amount = Some(data.budgeted_amount);
            }
        }

        transaction.actual_date = actual_date;
        transaction.actual_amount = actual_amount;
        transaction.status = data.status.clone();
        transaction.notes = data.notes.clone();

        match &data.recurrence {
            TransactionRecurrenceAction::Clear => {
                transaction.set_recurrence(None);
                transaction.recurrence_series_id = None;
            }
            TransactionRecurrenceAction::Set(recurrence) => {
                transaction.set_recurrence(Some(recurrence.clone()));
            }
            TransactionRecurrenceAction::Keep => {}
        }
    }

    fn apply_transaction_creation(
        &mut self,
        data: TransactionFormData,
        simulation: Option<&str>,
    ) -> CommandResult {
        let mut transaction = Transaction::new(
            data.from_account,
            data.to_account,
            data.category_id,
            data.scheduled_date,
            data.budgeted_amount,
        );
        Self::populate_transaction_from_form(&mut transaction, &data);

        let summary =
            self.with_ledger(|ledger| Ok(self.transaction_summary_line(ledger, &transaction)))?;

        if let Some(name) = simulation {
            self.with_ledger_mut(|ledger| {
                ledger
                    .add_simulation_transaction(name, transaction)
                    .map_err(CommandError::from_core)
            })?;
            cli_io::print_success(format!(
                "Transaction saved to simulation `{}`: {}",
                name, summary
            ));
        } else {
            let id = self.with_ledger_mut(|ledger| {
                TransactionService::add(ledger, transaction).map_err(CommandError::from)
            })?;
            let summary = self.with_ledger(|ledger| {
                let txn = ledger
                    .transaction(id)
                    .expect("transaction just added should exist");
                Ok(self.transaction_summary_line(ledger, txn))
            })?;
            cli_io::print_success(format!("Transaction saved: {}", summary));
        }
        Ok(())
    }

    fn apply_transaction_update(&mut self, data: TransactionFormData) -> CommandResult {
        let txn_id = data.id.ok_or_else(|| {
            CommandError::InvalidArguments("transaction identifier missing".into())
        })?;
        self.with_ledger_mut(|ledger| {
            TransactionService::update(ledger, txn_id, |transaction| {
                Self::populate_transaction_from_form(transaction, &data);
            })
            .map_err(CommandError::from)
        })?;
        let summary = self.with_ledger(|ledger| {
            let txn = ledger
                .transaction(txn_id)
                .expect("transaction should exist after update");
            Ok(self.transaction_summary_line(ledger, txn))
        })?;
        cli_io::print_success(format!("Transaction updated: {}", summary));
        Ok(())
    }

    fn remove_transaction_by_index(&mut self, index: usize) -> CommandResult {
        let (transaction_id, summary) = self.with_ledger(|ledger| {
            let txn = ledger.transactions.get(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            let summary = self.transaction_summary_line(ledger, txn);
            Ok((txn.id, summary))
        })?;
        self.with_ledger_mut(|ledger| {
            TransactionService::remove(ledger, transaction_id).map_err(CommandError::from)
        })?;
        cli_io::print_success(format!("Transaction removed: {}", summary));
        Ok(())
    }

    fn display_transaction(&self, index: usize) -> CommandResult {
        self.with_ledger(|ledger| {
            let txn = ledger.transactions.get(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;

            cli_io::print_info(format!("Transaction [{}]", index));
            let route = self.describe_transaction_route(ledger, txn);
            cli_io::print_info(format!("Route: {}", route));
            let category = txn
                .category_id
                .and_then(|id| self.lookup_category_name(ledger, id))
                .unwrap_or_else(|| "Uncategorized".into());
            cli_io::print_info(format!("Category: {}", category));
            cli_io::print_info(format!(
                "Scheduled: {}",
                self.format_date(ledger, txn.scheduled_date)
            ));
            let budget = format_currency_value(
                txn.budgeted_amount,
                &ledger.transaction_currency(txn),
                &ledger.locale,
                &ledger.format,
            );
            cli_io::print_info(format!("Budgeted: {}", budget));
            if txn.actual_amount.is_some() || txn.actual_date.is_some() {
                let amount_label = txn
                    .actual_amount
                    .map(|value| {
                        format_currency_value(
                            value,
                            &ledger.transaction_currency(txn),
                            &ledger.locale,
                            &ledger.format,
                        )
                    })
                    .unwrap_or_else(|| "-".into());
                let date_label = txn
                    .actual_date
                    .map(|date| self.format_date(ledger, date))
                    .unwrap_or_else(|| "-".into());
                cli_io::print_info(format!("Actual: {} on {}", amount_label, date_label));
            }
            cli_io::print_info(format!("Status: {:?}", txn.status));
            if let Some(hint) = self.transaction_recurrence_hint(txn) {
                cli_io::print_info(format!("Recurrence: {}", hint));
            } else if txn.recurrence.is_some() || txn.recurrence_series_id.is_some() {
                cli_io::print_info("Recurrence: linked instance");
            }
            if let Some(notes) = &txn.notes {
                if !notes.trim().is_empty() {
                    cli_io::print_info(format!("Notes: {}", notes));
                }
            }
            Ok(())
        })?;
        self.await_menu_escape()
    }

    pub(crate) fn run_account_add_wizard(&mut self) -> CommandResult {
        self.ensure_base_mode("Account creation")?;
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: add account <name> <kind>".into(),
            ));
        }

        let (existing_names, category_options) = self.with_ledger(|ledger| {
            let names: HashSet<String> = ledger.accounts.iter().map(|a| a.name.clone()).collect();
            let categories = self.account_category_options(ledger);
            Ok((names, categories))
        })?;

        let wizard = AccountWizard::new_create(existing_names, category_options);
        Banner::render(self);
        let mut interaction = WizardInteraction::new();
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                cli_io::print_info("Account creation cancelled.");
                Ok(())
            }
            FormResult::Completed(data) => self.apply_account_form(data),
        }
    }

    pub(crate) fn run_account_edit_wizard(&mut self, index: usize) -> CommandResult {
        self.ensure_base_mode("Account editing")?;
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: account edit <index>".into(),
            ));
        }

        let (existing_names, category_options, initial) = self.with_ledger(|ledger| {
            if index >= ledger.accounts.len() {
                return Err(CommandError::InvalidArguments(
                    "account index out of range".into(),
                ));
            }
            let account = &ledger.accounts[index];
            let names: HashSet<String> = ledger.accounts.iter().map(|a| a.name.clone()).collect();
            let categories = self.account_category_options(ledger);
            let initial = AccountInitialData {
                id: account.id,
                name: account.name.clone(),
                kind: account.kind.clone(),
                category_id: account.category_id,
                opening_balance: account.opening_balance,
                notes: account.notes.clone(),
            };
            Ok((names, categories, initial))
        })?;

        let wizard = AccountWizard::new_edit(existing_names, initial, category_options);
        Banner::render(self);
        let mut interaction = WizardInteraction::new();
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                cli_io::print_info("Account update cancelled.");
                Ok(())
            }
            FormResult::Completed(data) => self.apply_account_form(data),
        }
    }

    pub(crate) fn run_category_add_wizard(&mut self) -> CommandResult {
        self.ensure_base_mode("Category creation")?;
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: add category <name> <kind>".into(),
            ));
        }

        let (existing_names, parent_options) = self.with_ledger(|ledger| {
            let names: HashSet<String> = ledger.categories.iter().map(|c| c.name.clone()).collect();
            let parents = self.category_parent_options(ledger, &HashSet::new());
            Ok((names, parents))
        })?;

        let wizard = CategoryWizard::new_create(existing_names, parent_options);
        Banner::render(self);
        let mut interaction = WizardInteraction::new();
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                cli_io::print_info("Category creation cancelled.");
                Ok(())
            }
            FormResult::Completed(data) => self.apply_category_form(data),
        }
    }

    pub(crate) fn run_category_edit_wizard(&mut self, index: usize) -> CommandResult {
        self.ensure_base_mode("Category editing")?;
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: category edit <index>".into(),
            ));
        }

        let (existing_names, parent_options, initial, allow_kind_change, allow_custom_change) =
            self.with_ledger(|ledger| {
                if index >= ledger.categories.len() {
                    return Err(CommandError::InvalidArguments(
                        "category index out of range".into(),
                    ));
                }
                let category = &ledger.categories[index];
                let names: HashSet<String> =
                    ledger.categories.iter().map(|c| c.name.clone()).collect();
                let mut exclude = self.category_descendants(ledger, category.id);
                exclude.insert(category.id);
                let parents = self.category_parent_options(ledger, &exclude);
                let initial = CategoryInitialData {
                    id: category.id,
                    name: category.name.clone(),
                    kind: category.kind.clone(),
                    parent_id: category.parent_id,
                    is_custom: category.is_custom,
                    notes: category.notes.clone(),
                };
                let allow_kind_change = category.is_custom;
                let allow_custom_change = category.is_custom;
                Ok((
                    names,
                    parents,
                    initial,
                    allow_kind_change,
                    allow_custom_change,
                ))
            })?;

        if !allow_kind_change || !allow_custom_change {
            cli_io::print_info(
                "Note: predefined categories cannot change their type or custom flag.",
            );
        }

        let wizard = CategoryWizard::new_edit(
            existing_names,
            initial,
            parent_options,
            allow_kind_change,
            allow_custom_change,
        );
        Banner::render(self);
        let mut interaction = WizardInteraction::new();
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                cli_io::print_info("Category update cancelled.");
                Ok(())
            }
            FormResult::Completed(data) => self.apply_category_form(data),
        }
    }

    pub(crate) fn transaction_index_from_arg(
        &self,
        arg: Option<&str>,
        usage: &str,
        prompt: &str,
    ) -> Result<Option<usize>, CommandError> {
        if let Some(raw) = arg {
            let index = raw.parse::<usize>().map_err(|_| {
                CommandError::InvalidArguments("transaction_index must be numeric".into())
            })?;
            Ok(Some(index))
        } else if self.can_prompt() {
            self.select_transaction_index(prompt)
        } else {
            Err(CommandError::InvalidArguments(usage.into()))
        }
    }

    #[cfg(test)]
    fn set_selection_choices(&mut self, choices: impl IntoIterator<Item = Option<usize>>) {
        let override_data = self
            .selection_override
            .get_or_insert_with(SelectionOverride::default);
        for choice in choices {
            override_data.push(choice);
        }
    }

    #[cfg(test)]
    fn reset_selection_choices(&mut self) {
        if let Some(override_data) = &self.selection_override {
            override_data.clear();
        }
    }

    fn set_ledger(&mut self, ledger: Ledger, path: Option<PathBuf>, name: Option<String>) {
        {
            let mut manager = self.manager_mut();
            manager.set_current(ledger, path.clone(), name);
        }
        self.ledger_path = path;
        self.active_simulation_name = None;
        self.current_simulation = None;
    }

    pub(crate) fn command(&self, name: &str) -> Option<&CommandEntry> {
        self.registry.get(name)
    }

    pub(crate) fn run_new_ledger_interactive(&mut self) -> CommandResult {
        let name: String = Input::with_theme(&self.theme)
            .with_prompt("Ledger name")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.trim().is_empty() {
                    Err("Name cannot be empty")
                } else {
                    Ok(())
                }
            })
            .interact_text()
            .map_err(CommandError::from)?;

        let period = self.prompt_budget_period()?;
        let ledger = Ledger::new(name.clone(), period);
        self.set_ledger(ledger, None, Some(name));
        cli_io::print_success("New ledger created.");
        Ok(())
    }

    fn prompt_budget_amount(&self, prompt: &str) -> Result<f64, CommandError> {
        Input::<f64>::with_theme(&self.theme)
            .with_prompt(prompt)
            .validate_with(|value: &f64| -> Result<(), &str> {
                if *value <= 0.0 {
                    Err("Amount must be greater than 0")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(CommandError::from)
    }

    fn prompt_budget_period(&self) -> Result<BudgetPeriod, CommandError> {
        let interval = self.prompt_time_interval(None)?;
        Ok(BudgetPeriod(interval))
    }

    fn prompt_category_budget_period(
        &self,
        default: CategoryBudgetPeriod,
    ) -> Result<CategoryBudgetPeriod, CommandError> {
        let options = ["Monthly", "Weekly", "Daily", "Yearly", "Custom..."];
        let mut default_index = match default {
            CategoryBudgetPeriod::Monthly => 0,
            CategoryBudgetPeriod::Weekly => 1,
            CategoryBudgetPeriod::Daily => 2,
            CategoryBudgetPeriod::Yearly => 3,
            CategoryBudgetPeriod::Custom(_) => options.len() - 1,
        };
        default_index = default_index.min(options.len() - 1);
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Budget period")
            .items(&options)
            .default(default_index)
            .interact()
            .map_err(CommandError::from)?;
        if selection == options.len() - 1 {
            let mut custom_input = Input::<u32>::with_theme(&self.theme)
                .with_prompt("Custom period length (days)")
                .validate_with(|value: &u32| -> Result<(), &str> {
                    if *value == 0 {
                        Err("Value must be greater than 0")
                    } else {
                        Ok(())
                    }
                });
            if let CategoryBudgetPeriod::Custom(days) = default {
                custom_input = custom_input.with_initial_text(days.to_string());
            }
            let days = custom_input.interact().map_err(CommandError::from)?;
            return Ok(CategoryBudgetPeriod::Custom(days));
        }
        Ok(match selection {
            0 => CategoryBudgetPeriod::Monthly,
            1 => CategoryBudgetPeriod::Weekly,
            2 => CategoryBudgetPeriod::Daily,
            3 => CategoryBudgetPeriod::Yearly,
            _ => CategoryBudgetPeriod::Monthly,
        })
    }

    fn prompt_time_interval(
        &self,
        defaults: Option<&TimeInterval>,
    ) -> Result<TimeInterval, CommandError> {
        let options = interval_options();
        let custom_index = options.len() - 1;
        let mut default_selection = 0;
        let mut custom_defaults: Option<&TimeInterval> = None;
        if let Some(interval) = defaults {
            default_selection = match (interval.every, &interval.unit) {
                (1, TimeUnit::Month) => 0,
                (1, TimeUnit::Week) => 1,
                (1, TimeUnit::Day) => 2,
                (1, TimeUnit::Year) => 3,
                _ => {
                    custom_defaults = Some(interval);
                    custom_index
                }
            };
        }
        default_selection = default_selection.min(custom_index);

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Select interval")
            .items(options)
            .default(default_selection)
            .interact()
            .map_err(CommandError::from)?;

        if selection == custom_index {
            let mut every_input = Input::<u32>::with_theme(&self.theme)
                .with_prompt("Repeat every (number)")
                .validate_with(|value: &u32| -> Result<(), &str> {
                    if *value == 0 {
                        Err("Value must be greater than 0")
                    } else {
                        Ok(())
                    }
                });
            if let Some(defaults) = custom_defaults {
                every_input = every_input.with_initial_text(defaults.every.to_string());
            }
            let every: u32 = every_input.interact_text().map_err(CommandError::from)?;

            let units = ["Day", "Week", "Month", "Year"];
            let mut unit_default = 2;
            if let Some(defaults) = custom_defaults {
                unit_default = match defaults.unit {
                    TimeUnit::Day => 0,
                    TimeUnit::Week => 1,
                    TimeUnit::Month => 2,
                    TimeUnit::Year => 3,
                };
            }
            let unit_selection = Select::with_theme(&self.theme)
                .with_prompt("Time unit")
                .items(&units)
                .default(unit_default)
                .interact()
                .map_err(CommandError::from)?;
            let unit = match unit_selection {
                0 => TimeUnit::Day,
                1 => TimeUnit::Week,
                2 => TimeUnit::Month,
                _ => TimeUnit::Year,
            };

            Ok(TimeInterval { every, unit })
        } else {
            Ok(match options[selection].to_lowercase().as_str() {
                "monthly" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Month,
                },
                "weekly" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Week,
                },
                "daily" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Day,
                },
                "yearly" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Year,
                },
                _ => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Month,
                },
            })
        }
    }

    pub(crate) fn run_new_ledger_script(&mut self, args: &[&str]) -> CommandResult {
        if args.is_empty() {
            return Err(CommandError::InvalidArguments(
                "usage: ledger new <name> <period>".into(),
            ));
        }

        let name = args[0].to_string();
        let period_str = if args.len() > 1 {
            args[1..].join(" ")
        } else {
            "monthly".to_string()
        };
        let period = parse_period(&period_str)?;
        let ledger = Ledger::new(name.clone(), period);
        self.set_ledger(ledger, None, Some(name));
        cli_io::print_success("New ledger created.");
        Ok(())
    }

    pub(crate) fn load_ledger(&mut self, path: &Path) -> CommandResult {
        let report = self
            .manager_mut()
            .load_from_path(path)
            .map_err(CommandError::from_core)?;
        self.ledger_path = Some(path.to_path_buf());
        self.clear_active_simulation();
        cli_io::print_success(format!("Ledger loaded from {}.", path.display()));
        self.report_load(&report.warnings, &report.migrations);
        self.update_last_opened(None)?;
        Ok(())
    }

    pub(crate) fn save_to_path(&mut self, path: &Path) -> CommandResult {
        self.with_ledger(|ledger| {
            self.storage
                .save_to_path(ledger, path)
                .map_err(CommandError::from_core)
        })?;
        self.ledger_path = Some(path.to_path_buf());
        self.manager_mut().clear_name();
        cli_io::print_success(format!("Ledger saved to {}.", path.display()));
        self.update_last_opened(None)?;
        Ok(())
    }

    pub(crate) fn load_named_ledger(&mut self, name: &str) -> CommandResult {
        let report = {
            let mut manager = self.manager_mut();
            manager.load(name)
        }
        .map_err(CommandError::from_core)?;
        let path = self.storage.ledger_path(name);
        self.ledger_path = Some(path.clone());
        self.clear_active_simulation();
        cli_io::print_success(format!("Ledger `{}` loaded from {}.", name, path.display()));
        self.report_load(&report.warnings, &report.migrations);
        self.update_last_opened(Some(name))?;
        Ok(())
    }

    pub(crate) fn save_named_ledger(&mut self, name: &str) -> CommandResult {
        {
            let mut manager = self.manager_mut();
            manager.save_as(name).map_err(CommandError::from_core)?;
        }
        let path = self.storage.ledger_path(name);
        self.ledger_path = Some(path.clone());
        cli_io::print_success(format!("Ledger `{}` saved to {}.", name, path.display()));
        self.update_last_opened(Some(name))?;
        Ok(())
    }

    pub(crate) fn create_backup(&mut self, name: &str) -> CommandResult {
        let current = self.require_named_ledger()?;
        if !current.eq_ignore_ascii_case(name) {
            return Err(CommandError::InvalidArguments(format!(
                "`{}` is not the active ledger (current: `{}`).",
                name, current
            )));
        }
        self.manager()
            .backup(None)
            .map_err(CommandError::from_core)?;
        cli_io::print_success("Backup created.");
        Ok(())
    }

    pub(crate) fn list_backups(&self, name: &str) -> CommandResult {
        let backups = self
            .manager()
            .list_backups(name)
            .map_err(CommandError::from_core)?;
        if backups.is_empty() {
            cli_io::print_warning("No backups available.");
            return Ok(());
        }
        cli_io::print_info("Available backups:");
        for (idx, backup_name) in backups.iter().enumerate() {
            let description = format_backup_label(backup_name);
            cli_io::print_info(format!("  {:>2}. {}", idx + 1, description));
        }
        self.await_menu_escape()
    }

    pub(crate) fn restore_backup(&mut self, name: &str, reference: &str) -> CommandResult {
        let backups = self
            .manager()
            .list_backups(name)
            .map_err(CommandError::from_core)?;
        if backups.is_empty() {
            return Err(CommandError::InvalidArguments(
                "no backups available to restore".into(),
            ));
        }
        let target = if let Ok(index_raw) = reference.parse::<usize>() {
            let index = if index_raw > 0 {
                index_raw - 1
            } else {
                index_raw
            };
            backups
                .get(index)
                .ok_or_else(|| {
                    CommandError::InvalidArguments(format!(
                        "backup index {} out of range",
                        reference
                    ))
                })?
                .clone()
        } else {
            backups
                .iter()
                .find(|candidate| candidate.contains(reference))
                .cloned()
                .ok_or_else(|| {
                    CommandError::InvalidArguments(format!(
                        "no backup matches reference `{}`",
                        reference
                    ))
                })?
        };
        self.restore_backup_from_name(name, target)
    }

    pub(crate) fn restore_backup_from_name(
        &mut self,
        name: &str,
        backup_name: String,
    ) -> CommandResult {
        let confirm = if self.mode == CliMode::Interactive {
            cli_io::confirm_action(&format!(
                "Restore ledger `{}` from backup `{}`?",
                name, backup_name
            ))
            .map_err(CommandError::from)?
        } else {
            true
        };
        if !confirm {
            cli_io::print_info("Operation cancelled.");
            return Ok(());
        }
        let report = self
            .manager_mut()
            .restore_backup(name, &backup_name)
            .map_err(CommandError::from_core)?;
        let path = self.storage.ledger_path(name);
        self.ledger_path = Some(path.clone());
        self.clear_active_simulation();
        self.report_load(&report.warnings, &report.migrations);
        cli_io::print_success(format!(
            "Ledger `{}` loaded from backup `{}`.",
            name, backup_name
        ));
        self.update_last_opened(Some(name))?;
        Ok(())
    }

    pub(crate) fn backup_app_config(&mut self, note: Option<String>) -> CommandResult {
        let config = self.config_read();
        let manager = self.config_manager();
        let file_name = manager
            .backup(&config, note.as_deref())
            .map_err(CommandError::from_core)?;
        cli_io::print_success(format!("Configuration backup saved: {}", file_name));
        Ok(())
    }

    pub(crate) fn list_config_backups(&self) -> CommandResult {
        let manager = self.config_manager();
        let backups = manager.list_backups().map_err(CommandError::from_core)?;
        if backups.is_empty() {
            cli_io::print_warning("No configuration backups found.");
            return Ok(());
        }
        cli_io::print_info("Available configuration backups:");
        for (idx, name) in backups.iter().enumerate() {
            cli_io::print_info(format!("  {:>2}. {}", idx + 1, format_backup_label(name)));
        }
        self.await_menu_escape()
    }

    pub(crate) fn restore_config_by_reference(&mut self, reference: &str) -> CommandResult {
        let target = {
            let manager = self.config_manager();
            let backups = manager.list_backups().map_err(CommandError::from_core)?;
            if backups.is_empty() {
                return Err(CommandError::InvalidArguments(
                    "no configuration backups available".into(),
                ));
            }
            if let Ok(index_raw) = reference.parse::<usize>() {
                let index = if index_raw > 0 {
                    index_raw - 1
                } else {
                    index_raw
                };
                backups
                    .get(index)
                    .ok_or_else(|| {
                        CommandError::InvalidArguments(format!(
                            "configuration backup index {} out of range",
                            reference
                        ))
                    })?
                    .clone()
            } else {
                backups
                    .iter()
                    .find(|candidate| candidate.contains(reference))
                    .cloned()
                    .ok_or_else(|| {
                        CommandError::InvalidArguments(format!(
                            "no configuration backup matches reference `{}`",
                            reference
                        ))
                    })?
            }
        };
        self.restore_config_from_name(target)
    }

    pub(crate) fn restore_config_from_name(&mut self, backup_name: String) -> CommandResult {
        let manager = self.config_manager();
        let restored = manager
            .restore(&backup_name)
            .map_err(CommandError::from_core)?;
        {
            let mut config = self.config_write();
            *config = restored;
            cli_io::apply_config(&config);
        }
        self.persist_config()?;
        cli_io::print_success(format!("Configuration restored from {}.", backup_name));
        Ok(())
    }

    pub(crate) fn add_account_script(&mut self, args: &[&str]) -> CommandResult {
        if self.active_simulation_name().is_some() {
            return Err(CommandError::InvalidArguments(
                "Leave simulation mode before editing accounts".into(),
            ));
        }
        if args.len() < 2 {
            return Err(CommandError::InvalidArguments(
                "usage: add account <name> <kind>".into(),
            ));
        }

        let name = args[0].to_string();
        let kind = parse_account_kind(args[1])?;
        let account = Account::new(name, kind);
        self.with_ledger_mut(|ledger| {
            ledger.add_account(account);
            Ok(())
        })?;
        cli_io::print_success("Account added.");
        Ok(())
    }

    pub(crate) fn add_category_script(&mut self, args: &[&str]) -> CommandResult {
        if self.active_simulation_name().is_some() {
            return Err(CommandError::InvalidArguments(
                "Leave simulation mode before editing categories".into(),
            ));
        }
        if args.len() < 2 {
            return Err(CommandError::InvalidArguments(
                "usage: add category <name> <kind>".into(),
            ));
        }

        let name = args[0].to_string();
        let kind = parse_category_kind(args[1])?;
        let category = Category::new(name, kind);
        self.with_ledger_mut(|ledger| {
            ledger.add_category(category);
            Ok(())
        })?;
        cli_io::print_success("Category added.");
        Ok(())
    }

    pub(crate) fn category_budget_set(&mut self, args: &[&str]) -> CommandResult {
        self.ensure_base_mode("Category budgets")?;
        if self.active_simulation_name().is_some() {
            return Err(CommandError::InvalidArguments(
                "Leave simulation mode before editing categories".into(),
            ));
        }

        let (positionals, period_arg) = split_period_flag(args);
        if period_arg.as_deref().is_some_and(|value| value.is_empty()) {
            return Err(CommandError::InvalidArguments(
                "missing value for --period".into(),
            ));
        }
        if positionals.len() > 2 {
            return Err(CommandError::InvalidArguments(
                "usage: category budget set <category_name> <amount> [--period <period>]".into(),
            ));
        }

        let mut positional_iter = positionals.iter();
        let category_arg = positional_iter.next().map(|value| value.as_str());
        let amount_arg = positional_iter.next().map(|value| value.as_str());

        let target = self.resolve_category_target(
            category_arg,
            "usage: category budget set <category_name> <amount> [--period <period>]",
            "Select a category to assign a budget to:",
        )?;
        let Some((category_id, category_name)) = target else {
            cli_io::print_info("Budget assignment cancelled.");
            return Ok(());
        };

        let amount = if let Some(raw) = amount_arg {
            parse_budget_amount(raw)?
        } else if self.can_prompt() {
            self.prompt_budget_amount("Budget amount")?
        } else {
            return Err(CommandError::InvalidArguments(
                "usage: category budget set <category_name> <amount> [--period <period>]".into(),
            ));
        };

        let should_prompt_period = period_arg.is_none()
            && self.can_prompt()
            && (category_arg.is_none() || amount_arg.is_none());
        let mut used_default_period = false;
        let period_value = period_arg.clone();
        let period = if should_prompt_period {
            self.prompt_category_budget_period(self.config_default_category_period())?
        } else if let Some(value) = period_value {
            if value.eq_ignore_ascii_case("default") {
                used_default_period = true;
                self.config_default_category_period()
            } else {
                parse_category_budget_period_str(&value)?
            }
        } else {
            used_default_period = true;
            self.config_default_category_period()
        };

        self.with_ledger_mut(|ledger| {
            let category = ledger.category_mut(category_id).ok_or_else(|| {
                CommandError::InvalidArguments(format!(
                    "category `{}` no longer exists.",
                    category_name
                ))
            })?;
            category.set_budget(amount, period.clone(), None);
            Ok(())
        })?;

        let budget_label = self.with_ledger(|ledger| {
            let amount_label = self.format_amount(ledger, amount);
            Ok((
                amount_label,
                self.describe_budget_period_label(ledger, &period, None),
            ))
        })?;
        cli_io::print_success(format!(
            "Budget for `{}` set to {} ({})",
            category_name, budget_label.0, budget_label.1
        ));
        if used_default_period {
            self.print_hint(
                "Hint: Change the default via `config set default_budget_period monthly`.",
            );
        }
        Ok(())
    }

    pub(crate) fn category_budget_clear(&mut self, args: &[&str]) -> CommandResult {
        self.ensure_base_mode("Category budgets")?;
        if args.len() > 1 {
            return Err(CommandError::InvalidArguments(
                "usage: category budget clear <category_name>".into(),
            ));
        }
        let target = self.resolve_category_target(
            args.get(0).copied(),
            "usage: category budget clear <category_name>",
            "Select a category to clear:",
        )?;
        let Some((category_id, category_name)) = target else {
            cli_io::print_info("Budget removal cancelled.");
            return Ok(());
        };

        let removed = self.with_ledger_mut(|ledger| {
            let category = ledger.category_mut(category_id).ok_or_else(|| {
                CommandError::InvalidArguments(format!(
                    "category `{}` no longer exists.",
                    category_name
                ))
            })?;
            let had_budget = category.budget.is_some();
            if had_budget {
                category.clear_budget();
            }
            Ok(had_budget)
        })?;

        if removed {
            cli_io::print_success(format!("Budget cleared for `{}`.", category_name));
        } else {
            cli_io::print_info(format!(
                "Category `{}` has no budget assigned.",
                category_name
            ));
        }
        Ok(())
    }

    pub(crate) fn category_budget_show(&self, args: &[&str]) -> CommandResult {
        if args.len() > 1 {
            return Err(CommandError::InvalidArguments(
                "usage: category budget show [<category_name>]".into(),
            ));
        }
        let name_filter = args
            .first()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let data = self.with_ledger(|ledger| {
            let mut statuses: Vec<CategoryBudgetStatus> = ledger
                .category_budget_statuses_current()
                .into_iter()
                .filter(|status| status.budget.is_some())
                .collect();
            if statuses.is_empty() {
                if let Some(filter) = name_filter {
                    return Err(CommandError::InvalidArguments(format!(
                        "category `{}` has no budget configured",
                        filter
                    )));
                }
                return Ok(None);
            }
            if let Some(filter) = name_filter {
                if let Some(status) = statuses
                    .into_iter()
                    .find(|status| status.name.eq_ignore_ascii_case(filter))
                {
                    let row = self.category_budget_row(ledger, &status);
                    let heading = format!("Category Budget: {}", status.name);
                    return Ok(Some((heading, vec![row])));
                } else {
                    return Err(CommandError::InvalidArguments(format!(
                        "category `{}` has no budget configured",
                        filter
                    )));
                }
            }
            statuses.sort_by(|a, b| a.name.cmp(&b.name));
            let rows: Vec<Vec<String>> = statuses
                .iter()
                .map(|status| self.category_budget_row(ledger, status))
                .collect();
            Ok(Some((
                "Category Budgets (current period)".to_string(),
                rows,
            )))
        })?;

        let displayed = match data {
            None => {
                cli_io::print_warning("No category budgets configured.");
                false
            }
            Some((heading, rows)) => {
                Formatter::new().print_header(heading);
                output_table(
                    &[
                        "Category",
                        "Budget",
                        "Spent",
                        "Remaining",
                        "Period",
                        "Status",
                    ],
                    &rows,
                );
                true
            }
        };
        if name_filter.is_none() {
            self.print_hint(
                "Hint: Use `category budget set <name> <amount>` to add or update a budget.",
            );
        }
        if displayed {
            self.await_menu_escape()?;
        }
        Ok(())
    }

    pub(crate) fn add_transaction_script(&mut self, args: &[&str]) -> CommandResult {
        if args.len() < 4 {
            return Err(CommandError::InvalidArguments(
                "usage: add transaction <from_account_index> <to_account_index> <YYYY-MM-DD> <amount>"
                    .into(),
            ));
        }

        let sim = self.active_simulation_name().map(|s| s.to_string());

        let from_index: usize = args[0].parse().map_err(|_| {
            CommandError::InvalidArguments("from_account_index must be numeric".into())
        })?;
        let to_index: usize = args[1].parse().map_err(|_| {
            CommandError::InvalidArguments("to_account_index must be numeric".into())
        })?;
        let date = NaiveDate::parse_from_str(args[2], "%Y-%m-%d")
            .map_err(|_| CommandError::InvalidArguments("invalid date".into()))?;
        let amount: f64 = args[3]
            .parse()
            .map_err(|_| CommandError::InvalidArguments("invalid amount".into()))?;

        let (from_id, to_id) = self.with_ledger(|ledger| {
            if ledger.accounts.is_empty() {
                return Err(CommandError::Message(
                    "Add at least one account before creating transactions".into(),
                ));
            }
            if from_index >= ledger.accounts.len() || to_index >= ledger.accounts.len() {
                return Err(CommandError::InvalidArguments(
                    "account indices out of range".into(),
                ));
            }
            Ok((ledger.accounts[from_index].id, ledger.accounts[to_index].id))
        })?;

        let transaction = Transaction::new(from_id, to_id, None, date, amount);
        let summary =
            self.with_ledger(|ledger| Ok(self.transaction_summary_line(ledger, &transaction)))?;

        if let Some(sim_name) = sim {
            self.with_ledger_mut(|ledger| {
                ledger
                    .add_simulation_transaction(&sim_name, transaction)
                    .map_err(CommandError::from_core)
            })?;
            cli_io::print_success(format!(
                "Transaction saved to simulation `{}`: {}",
                sim_name, summary
            ));
        } else {
            let id = self.with_ledger_mut(|ledger| {
                TransactionService::add(ledger, transaction).map_err(CommandError::from)
            })?;
            let summary = self.with_ledger(|ledger| {
                let txn = ledger
                    .transaction(id)
                    .expect("transaction just added should exist");
                Ok(self.transaction_summary_line(ledger, txn))
            })?;
            cli_io::print_success(format!("Transaction saved: {}", summary));
        }
        Ok(())
    }

    fn run_transaction_add_wizard(&mut self, simulation: Option<&str>) -> CommandResult {
        let (accounts, categories, min_date) = self.with_ledger(|ledger| {
            if ledger.accounts.is_empty() {
                return Err(CommandError::Message(
                    "Add at least one account before creating transactions".into(),
                ));
            }
            let accounts = self.transaction_account_options(ledger);
            let categories = self.account_category_options(ledger);
            let min_date = ledger.created_at.date_naive();
            Ok((accounts, categories, min_date))
        })?;
        let today = Utc::now().date_naive();
        let default_status = if simulation.is_some() {
            TransactionStatus::Simulated
        } else {
            TransactionStatus::Planned
        };
        let wizard =
            TransactionWizard::new_create(accounts, categories, today, min_date, default_status);
        Banner::render(self);
        let mut interaction = WizardInteraction::new();
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                cli_io::print_info("Transaction creation cancelled.");
                Ok(())
            }
            FormResult::Completed(data) => self.apply_transaction_creation(data, simulation),
        }
    }

    fn run_transaction_edit_wizard(&mut self, index: usize) -> CommandResult {
        self.ensure_base_mode("Transaction editing")?;
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: transaction edit <index>".into(),
            ));
        }
        let (accounts, categories, initial, created_at) = self.with_ledger(|ledger| {
            if index >= ledger.transactions.len() {
                return Err(CommandError::InvalidArguments(
                    "transaction index out of range".into(),
                ));
            }
            let txn = ledger.transactions[index].clone();
            let accounts = self.transaction_account_options(ledger);
            let categories = self.account_category_options(ledger);
            let created_at = ledger.created_at;
            let initial = TransactionInitialData {
                id: txn.id,
                from_account: txn.from_account,
                to_account: txn.to_account,
                category_id: txn.category_id,
                scheduled_date: txn.scheduled_date,
                actual_date: txn.actual_date,
                budgeted_amount: txn.budgeted_amount,
                actual_amount: txn.actual_amount,
                recurrence: txn.recurrence.clone(),
                status: txn.status.clone(),
                notes: txn.notes.clone(),
            };
            Ok((accounts, categories, initial, created_at))
        })?;
        let today = Utc::now().date_naive();
        let min_date = created_at.date_naive();
        let wizard = TransactionWizard::new_edit(accounts, categories, today, min_date, initial);
        Banner::render(self);
        let mut interaction = WizardInteraction::new();
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                cli_io::print_info("Transaction update cancelled.");
                Ok(())
            }
            FormResult::Completed(data) => self.apply_transaction_update(data),
        }
    }

    pub(crate) fn transaction_add(&mut self, args: &[&str]) -> CommandResult {
        if args.is_empty() {
            if self.mode == CliMode::Interactive {
                let sim = self.active_simulation_name().map(|s| s.to_string());
                self.run_transaction_add_wizard(sim.as_deref())
            } else {
                Err(CommandError::InvalidArguments(
                    "usage: transaction add <from_account_index> <to_account_index> <YYYY-MM-DD> <amount>"
                        .into(),
                ))
            }
        } else {
            self.add_transaction_script(args)
        }
    }

    pub(crate) fn transaction_edit(&mut self, args: &[&str]) -> CommandResult {
        if self.with_ledger(|ledger| Ok(ledger.transactions.is_empty()))? {
            cli_io::print_warning("No transactions available.");
            return Ok(());
        }
        if args.len() > 1 {
            return Err(CommandError::InvalidArguments(
                "usage: transaction edit <index>".into(),
            ));
        }
        let usage = "usage: transaction edit <index>";
        let prompt = "Select a transaction to edit:";
        let selection = self.transaction_index_from_arg(args.first().copied(), usage, prompt)?;
        let Some(index) = selection else {
            return Ok(());
        };
        self.run_transaction_edit_wizard(index)
    }

    pub(crate) fn transaction_remove(&mut self, args: &[&str]) -> CommandResult {
        self.ensure_base_mode("Transaction removal")?;
        if self.with_ledger(|ledger| Ok(ledger.transactions.is_empty()))? {
            cli_io::print_warning("No transactions available.");
            return Ok(());
        }
        if args.len() > 1 {
            return Err(CommandError::InvalidArguments(
                "usage: transaction remove <index>".into(),
            ));
        }
        let usage = "usage: transaction remove <index>";
        let prompt = "Select a transaction to remove:";
        let selection = self.transaction_index_from_arg(args.first().copied(), usage, prompt)?;
        let Some(index) = selection else {
            return Ok(());
        };
        self.remove_transaction_by_index(index)
    }

    pub(crate) fn transaction_show(&mut self, args: &[&str]) -> CommandResult {
        if self.with_ledger(|ledger| Ok(ledger.transactions.is_empty()))? {
            cli_io::print_warning("No transactions available.");
            return Ok(());
        }
        if args.len() > 1 {
            return Err(CommandError::InvalidArguments(
                "usage: transaction show <index>".into(),
            ));
        }
        let usage = "usage: transaction show <index>";
        let prompt = "Select a transaction to show:";
        let selection = self.transaction_index_from_arg(args.first().copied(), usage, prompt)?;
        let Some(index) = selection else {
            return Ok(());
        };
        self.display_transaction(index)
    }

    pub(crate) fn transaction_complete_internal(
        &mut self,
        args: &[&str],
        usage: &str,
        prompt: &str,
    ) -> CommandResult {
        self.ensure_base_mode("Completion")?;
        if self.with_ledger(|ledger| Ok(ledger.transactions.is_empty()))? {
            cli_io::print_warning("No transactions available.");
            return Ok(());
        }
        let selection = self.transaction_index_from_arg(args.first().copied(), usage, prompt)?;
        let Some(idx) = selection else {
            return Ok(());
        };

        let (scheduled_default, budget_default) = self.with_ledger(|ledger| {
            let txn = ledger.transactions.get(idx).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            Ok((
                txn.scheduled_date,
                txn.actual_amount.unwrap_or(txn.budgeted_amount),
            ))
        })?;

        let actual_date = if let Some(raw) = args.get(1) {
            parse_date(raw)?
        } else if self.mode == CliMode::Interactive {
            let prompt = format!("Completion date for transaction {} (YYYY-MM-DD)", idx);
            let input = Input::<String>::with_theme(&self.theme)
                .with_prompt(prompt)
                .with_initial_text(scheduled_default.to_string())
                .interact_text()
                .map_err(CommandError::from)?;
            parse_date(input.trim())?
        } else {
            return Err(CommandError::InvalidArguments(usage.into()));
        };

        let amount: f64 = if let Some(raw) = args.get(2) {
            raw.parse()
                .map_err(|_| CommandError::InvalidArguments("amount must be numeric".into()))?
        } else if self.mode == CliMode::Interactive {
            let prompt = format!("Actual amount for transaction {}", idx);
            let input = Input::<String>::with_theme(&self.theme)
                .with_prompt(prompt)
                .with_initial_text(format!("{:.2}", budget_default))
                .interact_text()
                .map_err(CommandError::from)?;
            input
                .trim()
                .parse()
                .map_err(|_| CommandError::InvalidArguments("amount must be numeric".into()))?
        } else {
            return Err(CommandError::InvalidArguments(usage.into()));
        };

        let txn_id = self.with_ledger(|ledger| {
            let txn = ledger.transactions.get(idx).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            Ok(txn.id)
        })?;

        self.with_ledger_mut(|ledger| {
            TransactionService::update(ledger, txn_id, |txn| {
                txn.mark_completed(actual_date, amount);
            })
            .map_err(CommandError::from)
        })?;
        cli_io::print_success(format!("Transaction {} marked completed", idx));
        Ok(())
    }

    pub(crate) fn transaction_complete(&mut self, args: &[&str]) -> CommandResult {
        self.transaction_complete_internal(
            args,
            "usage: transaction complete <transaction_index> <YYYY-MM-DD> <amount>",
            "Select a transaction to complete:",
        )
    }

    fn prompt_recurrence(
        &self,
        default_start: NaiveDate,
        existing: Option<&Recurrence>,
    ) -> Result<Recurrence, CommandError> {
        let start_default = existing.map(|r| r.start_date).unwrap_or(default_start);
        let start_input = Input::<String>::with_theme(&self.theme)
            .with_prompt("Start date (YYYY-MM-DD)")
            .with_initial_text(start_default.to_string());
        let start_raw = start_input.interact_text().map_err(CommandError::from)?;
        let start_date = parse_date(&start_raw)?;

        let interval = self.prompt_time_interval(existing.map(|r| &r.interval))?;
        let modes = [
            ("Fixed schedule", RecurrenceMode::FixedSchedule),
            ("After last performed", RecurrenceMode::AfterLastPerformed),
        ];
        let mode_default = existing
            .map(|r| match r.mode {
                RecurrenceMode::FixedSchedule => 0,
                RecurrenceMode::AfterLastPerformed => 1,
            })
            .unwrap_or(0);
        let mode_selection = Select::with_theme(&self.theme)
            .with_prompt("Recurrence mode")
            .items(&modes.iter().map(|(label, _)| *label).collect::<Vec<_>>())
            .default(mode_default)
            .interact()
            .map_err(CommandError::from)?;
        let mode = modes[mode_selection].1.clone();

        let end_options = ["No end", "End on date", "End after N occurrences"];
        let mut end_default = 0;
        let mut existing_end_date: Option<NaiveDate> = None;
        let mut existing_occurrences: Option<u32> = None;
        if let Some(recurrence) = existing {
            match recurrence.end {
                RecurrenceEnd::Never => end_default = 0,
                RecurrenceEnd::OnDate(date) => {
                    end_default = 1;
                    existing_end_date = Some(date);
                }
                RecurrenceEnd::AfterOccurrences(n) => {
                    end_default = 2;
                    existing_occurrences = Some(n);
                }
            }
        }
        let end_selection = Select::with_theme(&self.theme)
            .with_prompt("End condition")
            .items(&end_options)
            .default(end_default)
            .interact()
            .map_err(CommandError::from)?;
        let end = match end_selection {
            0 => RecurrenceEnd::Never,
            1 => {
                let default_text = existing_end_date.unwrap_or(start_date).to_string();
                let date_input = Input::<String>::with_theme(&self.theme)
                    .with_prompt("End date (YYYY-MM-DD)")
                    .with_initial_text(default_text)
                    .interact_text()
                    .map_err(CommandError::from)?;
                RecurrenceEnd::OnDate(parse_date(&date_input)?)
            }
            _ => {
                let mut count_input =
                    Input::<u32>::with_theme(&self.theme).with_prompt("Number of occurrences");
                if let Some(n) = existing_occurrences {
                    count_input = count_input.with_initial_text(n.to_string());
                }
                let count = count_input
                    .validate_with(|value: &u32| -> Result<(), &str> {
                        if *value == 0 {
                            Err("Value must be greater than zero")
                        } else {
                            Ok(())
                        }
                    })
                    .interact_text()
                    .map_err(CommandError::from)?;
                RecurrenceEnd::AfterOccurrences(count)
            }
        };

        let mut recurrence = Recurrence::new(start_date, interval, mode);
        recurrence.end = end;
        if let Some(existing) = existing {
            recurrence.series_id = existing.series_id;
            recurrence.exceptions = existing.exceptions.clone();
            recurrence.status = existing.status.clone();
            recurrence.last_generated = existing.last_generated;
            recurrence.last_completed = existing.last_completed;
            recurrence.generated_occurrences = existing.generated_occurrences;
            recurrence.next_scheduled = existing.next_scheduled;
        }
        Ok(recurrence)
    }

    pub(crate) fn list_accounts(&self) -> CommandResult {
        let rows = self.with_ledger(|ledger| {
            if ledger.accounts.is_empty() {
                return Ok(None);
            }
            let rows: Vec<Vec<String>> = ledger
                .accounts
                .iter()
                .map(|account| {
                    let category = account
                        .category_id
                        .and_then(|id| ledger.category(id))
                        .map(|cat| cat.name.clone())
                        .unwrap_or_else(|| "-".into());
                    vec![
                        account.name.clone(),
                        account.kind.to_string(),
                        category,
                        account.notes.as_deref().unwrap_or("-").to_string(),
                    ]
                })
                .collect();
            Ok(Some(rows))
        })?;

        match rows {
            Some(rows) => {
                Formatter::new().print_header("Accounts");
                output_table(&["Name", "Kind", "Category", "Notes"], &rows);
                self.await_menu_escape()
            }
            None => {
                cli_io::print_warning("No accounts defined.");
                Ok(())
            }
        }
    }

    pub(crate) fn list_categories(&self) -> CommandResult {
        let rows = self.with_ledger(|ledger| {
            if ledger.categories.is_empty() {
                return Ok(None);
            }
            let rows: Vec<Vec<String>> = ledger
                .categories
                .iter()
                .map(|category| {
                    let parent = category
                        .parent_id
                        .and_then(|id| ledger.category(id))
                        .map(|cat| cat.name.clone())
                        .unwrap_or_else(|| "-".into());
                    let budget_display = category
                        .budget
                        .as_ref()
                        .map(|budget| {
                            format!(
                                "{} ({})",
                                self.format_amount(ledger, budget.amount),
                                self.describe_budget_period_label(
                                    ledger,
                                    &budget.period,
                                    budget.reference_date
                                )
                            )
                        })
                        .unwrap_or_else(|| "-".into());
                    vec![
                        category.name.clone(),
                        category.kind.to_string(),
                        parent,
                        budget_display,
                        category.notes.as_deref().unwrap_or("-").to_string(),
                    ]
                })
                .collect();
            Ok(Some(rows))
        })?;

        match rows {
            Some(rows) => {
                Formatter::new().print_header("Categories");
                output_table(&["Name", "Kind", "Parent", "Budget", "Notes"], &rows);
                self.await_menu_escape()
            }
            None => {
                cli_io::print_warning("No categories defined.");
                Ok(())
            }
        }
    }

    pub(crate) fn list_transactions(&self) -> CommandResult {
        let rows = self.with_ledger(|ledger| {
            if ledger.transactions.is_empty() {
                return Ok(None);
            }
            let rows: Vec<Vec<String>> = ledger
                .transactions
                .iter()
                .map(|txn| {
                    let from = ledger
                        .account(txn.from_account)
                        .map(|acc| acc.name.clone())
                        .unwrap_or_else(|| "Unknown".into());
                    let to = ledger
                        .account(txn.to_account)
                        .map(|acc| acc.name.clone())
                        .unwrap_or_else(|| "Unknown".into());
                    vec![
                        self.format_date(ledger, txn.scheduled_date),
                        from,
                        to,
                        self.format_amount(ledger, txn.budgeted_amount),
                    ]
                })
                .collect();
            Ok(Some(rows))
        })?;

        match rows {
            Some(rows) => {
                Formatter::new().print_header("Transactions");
                output_table(&["Date", "From", "To", "Amount"], &rows);
                self.await_menu_escape()
            }
            None => {
                cli_io::print_warning("No transactions recorded.");
                Ok(())
            }
        }
    }

    pub(crate) fn show_budget_summary(&self, args: &[&str]) -> CommandResult {
        let displayed = self.with_ledger(|ledger| {
            let today = Utc::now().date_naive();

            let (simulation_name, remainder) =
                if !args.is_empty() && ledger.simulation(args[0]).is_some() {
                    (Some(args[0]), &args[1..])
                } else {
                    (None, args)
                };

            let (window, scope) = self.resolve_summary_window(ledger, remainder, today)?;

            if let Some(name) = simulation_name {
                let impact = SummaryService::summarize_simulation(ledger, name, window, scope)
                    .map_err(CommandError::from)?;
                self.print_simulation_impact(ledger, &impact);
                return Ok(true);
            }

            let summary = SummaryService::summarize_window(ledger, window, scope);
            let category_budgets = SummaryService::category_budget_summaries(ledger, window, scope);
            self.print_budget_summary(ledger, &summary, &category_budgets);
            Ok(true)
        })?;
        if displayed {
            self.await_menu_escape()?;
        }
        Ok(())
    }

    fn resolve_summary_window(
        &self,
        ledger: &Ledger,
        args: &[&str],
        today: NaiveDate,
    ) -> Result<(DateWindow, BudgetScope), CommandError> {
        if args.is_empty() {
            let window = ledger.budget_window_for(today);
            let scope = window.scope(today);
            return Ok((window, scope));
        }

        match args[0].to_lowercase().as_str() {
            "current" => {
                let window = ledger.budget_window_for(today);
                let scope = window.scope(today);
                Ok((window, scope))
            }
            "past" => {
                let offset = parse_positive_or_default(args.get(1), 1)? as i32;
                let base = ledger.budget_window_for(today);
                let window = base.shift(&ledger.budget_period.0, -offset);
                let scope = window.scope(today);
                Ok((window, scope))
            }
            "future" => {
                let offset = parse_positive_or_default(args.get(1), 1)? as i32;
                let base = ledger.budget_window_for(today);
                let window = base.shift(&ledger.budget_period.0, offset);
                let scope = window.scope(today);
                Ok((window, scope))
            }
            "custom" | "range" => {
                if args.len() < 3 {
                    return Err(CommandError::InvalidArguments(
                        "usage: summary custom <start> <end>".into(),
                    ));
                }
                let start = parse_date(args[1])?;
                let end = parse_date(args[2])?;
                let window = DateWindow::new(start, end).map_err(CommandError::from_core)?;
                Ok((window, BudgetScope::Custom))
            }
            other => Err(CommandError::InvalidArguments(format!(
                "unknown summary scope `{}`",
                other
            ))),
        }
    }

    pub(crate) fn resolve_forecast_window(
        &self,
        args: &[&str],
        today: NaiveDate,
    ) -> Result<DateWindow, CommandError> {
        if args.is_empty() {
            let end = today + Duration::days(90);
            return DateWindow::new(today, end).map_err(CommandError::from_core);
        }
        if matches!(args[0].to_lowercase().as_str(), "custom" | "range") {
            if args.len() < 3 {
                return Err(CommandError::InvalidArguments(
                    "usage: forecast custom <start YYYY-MM-DD> <end YYYY-MM-DD>".into(),
                ));
            }
            let start = parse_date(args[1])?;
            let end = parse_date(args[2])?;
            return DateWindow::new(start, end).map_err(CommandError::from_core);
        }
        let mut tokens = args;
        if !tokens.is_empty() && tokens[0].eq_ignore_ascii_case("next") {
            tokens = &tokens[1..];
        }
        if tokens.is_empty() {
            return Err(CommandError::InvalidArguments(
                "usage: forecast <number> <unit>".into(),
            ));
        }
        let interval_expr = tokens.join(" ");
        let interval = parse_time_interval_str(&interval_expr)?;
        let end = interval.add_to(today, 1);
        DateWindow::new(today, end).map_err(CommandError::from_core)
    }

    fn print_budget_summary(
        &self,
        ledger: &Ledger,
        summary: &BudgetSummary,
        category_budgets: &[CategoryBudgetSummary],
    ) {
        let end_display = summary
            .window
            .end
            .checked_sub_signed(Duration::days(1))
            .unwrap_or(summary.window.end);
        Formatter::new().print_header(format!(
            "{:?} {} → {}",
            summary.scope,
            self.format_date(ledger, summary.window.start),
            self.format_date(ledger, end_display)
        ));

        cli_io::print_info(format!(
            "Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, summary.totals.budgeted),
            self.format_amount(ledger, summary.totals.real),
            self.format_amount(ledger, summary.totals.remaining),
            self.format_amount(ledger, summary.totals.variance)
        ));

        if let Some(percent) = summary.totals.percent_used {
            cli_io::print_info(format!("Usage: {:.1}%", percent));
        }

        cli_io::print_info(format!("Status: {:?}", summary.totals.status));

        if summary.incomplete_transactions > 0 {
            cli_io::print_warning(format!(
                "{} incomplete transactions",
                summary.incomplete_transactions
            ));
        }

        if summary.orphaned_transactions > 0 {
            cli_io::print_warning(format!(
                "{} transactions reference unknown accounts or categories",
                summary.orphaned_transactions
            ));
        }

        if summary.per_category.is_empty() {
            cli_io::print_info("No category data for this window.");
        } else {
            cli_io::print_info("Categories:");
            for cat in summary.per_category.iter().take(5) {
                cli_io::print_info(format!(
                    "  {:<20} {} budgeted / {} real ({:?})",
                    cat.name,
                    self.format_amount(ledger, cat.totals.budgeted),
                    self.format_amount(ledger, cat.totals.real),
                    cat.totals.status
                ));
            }
            if summary.per_category.len() > 5 {
                cli_io::print_info(format!(
                    "  ... {} more categories",
                    summary.per_category.len() - 5
                ));
            }
        }

        if !summary.per_account.is_empty() {
            cli_io::print_info("Accounts:");
            for acct in summary.per_account.iter().take(5) {
                cli_io::print_info(format!(
                    "  {:<20} {} budgeted / {} real ({:?})",
                    acct.name,
                    self.format_amount(ledger, acct.totals.budgeted),
                    self.format_amount(ledger, acct.totals.real),
                    acct.totals.status
                ));
            }
            if summary.per_account.len() > 5 {
                cli_io::print_info(format!(
                    "  ... {} more accounts",
                    summary.per_account.len() - 5
                ));
            }
        }

        if !summary.disclosures.is_empty() {
            cli_io::print_info("Disclosures:");
            for note in &summary.disclosures {
                cli_io::print_info(format!("  - {}", note));
            }
        }

        self.print_category_budget_section(ledger, "Category Budgets", category_budgets);
    }

    fn print_category_budget_section(
        &self,
        ledger: &Ledger,
        heading: &str,
        budgets: &[CategoryBudgetSummary],
    ) {
        if budgets.is_empty() {
            cli_io::print_info(format!("{heading}: no category budgets configured."));
            return;
        }
        cli_io::print_info(heading);
        for summary in budgets.iter().take(8) {
            let icon = self.category_budget_status_icon(&summary.status);
            let utilization = summary
                .utilization_percent
                .map(|value| format!("{value:.0}%"))
                .unwrap_or_else(|| "-".into());
            cli_io::print_info(format!(
                "  {icon} {:<20} Budget {} | Spent {} | Remaining {} | Used {} | {}",
                summary.name,
                self.format_amount(ledger, summary.budget_amount),
                self.format_amount(ledger, summary.spent_amount),
                self.format_amount(ledger, summary.remaining_amount),
                utilization,
                self.describe_budget_period_label(ledger, &summary.period, summary.reference_date)
            ));
        }
        if budgets.len() > 8 {
            cli_io::print_info(format!("  ... {} more categories", budgets.len() - 8));
        }
    }

    fn category_budget_status_icon(&self, status: &BudgetStatus) -> &'static str {
        match status {
            BudgetStatus::OnTrack | BudgetStatus::UnderBudget => "✅",
            BudgetStatus::OverBudget => "❌",
            BudgetStatus::Empty => "–",
            BudgetStatus::Incomplete => "⚠️",
        }
    }

    fn print_simulation_impact(&self, ledger: &Ledger, impact: &SimulationBudgetImpact) {
        Formatter::new().print_header(format!("Simulation `{}`", impact.simulation_name));
        cli_io::print_info("Base totals:");
        cli_io::print_info(format!(
            "  Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, impact.base.totals.budgeted),
            self.format_amount(ledger, impact.base.totals.real),
            self.format_amount(ledger, impact.base.totals.remaining),
            self.format_amount(ledger, impact.base.totals.variance)
        ));
        cli_io::print_info("Simulated totals:");
        cli_io::print_info(format!(
            "  Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, impact.simulated.totals.budgeted),
            self.format_amount(ledger, impact.simulated.totals.real),
            self.format_amount(ledger, impact.simulated.totals.remaining),
            self.format_amount(ledger, impact.simulated.totals.variance)
        ));
        cli_io::print_info("Delta:");
        cli_io::print_info(format!(
            "  Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, impact.delta.budgeted),
            self.format_amount(ledger, impact.delta.real),
            self.format_amount(ledger, impact.delta.remaining),
            self.format_amount(ledger, impact.delta.variance)
        ));
        self.print_category_budget_section(
            ledger,
            "Category Budgets (Base)",
            &impact.category_budgets_base,
        );
        self.print_category_budget_section(
            ledger,
            "Category Budgets (Simulated)",
            &impact.category_budgets_simulated,
        );
    }

    pub(crate) fn print_forecast_report(
        &self,
        ledger: &Ledger,
        simulation: Option<&str>,
        report: &ForecastReport,
    ) {
        let window = report.forecast.window;
        let header = simulation
            .map(|name| format!("Forecast `{}`", name))
            .unwrap_or_else(|| "Forecast".to_string());
        let end_display = window
            .end
            .checked_sub_signed(Duration::days(1))
            .unwrap_or(window.end);
        Formatter::new().print_header(format!(
            "{header} {} → {}",
            self.format_date(ledger, window.start),
            self.format_date(ledger, end_display)
        ));

        let totals = &report.forecast.totals;
        let instance_count = report.forecast.instances.len();
        let generated_count = totals.generated;
        let existing_count = instance_count.saturating_sub(generated_count);
        let overdue = report
            .forecast
            .instances
            .iter()
            .filter(|inst| matches!(inst.status, ScheduledStatus::Overdue))
            .count();
        let pending = report
            .forecast
            .instances
            .iter()
            .filter(|inst| matches!(inst.status, ScheduledStatus::Pending))
            .count();
        let future = report
            .forecast
            .instances
            .iter()
            .filter(|inst| matches!(inst.status, ScheduledStatus::Future))
            .count();
        cli_io::print_info(format!(
            "Occurrences: {instance_count} total | {existing_count} already scheduled | {generated_count} projected"
        ));
        cli_io::print_info(format!(
            "Status mix: {overdue} overdue | {pending} pending | {future} future"
        ));
        cli_io::print_info(format!(
            "Projected totals: Inflow {} | Outflow {} | Net {}",
            self.format_amount(ledger, totals.projected_inflow),
            self.format_amount(ledger, totals.projected_outflow),
            self.format_amount(ledger, totals.net)
        ));
        cli_io::print_info(format!(
            "Budget impact: Budgeted {} | Real {} | Remaining {} | Variance {}",
            self.format_amount(ledger, report.summary.totals.budgeted),
            self.format_amount(ledger, report.summary.totals.real),
            self.format_amount(ledger, report.summary.totals.remaining),
            self.format_amount(ledger, report.summary.totals.variance)
        ));
        self.print_category_budget_section(
            ledger,
            "Category Budgets (Projected)",
            &report.category_budgets,
        );
        if !report.summary.disclosures.is_empty() {
            cli_io::print_info("Disclosures:");
            for note in &report.summary.disclosures {
                cli_io::print_info(format!("  - {}", note));
            }
        }

        if report.forecast.transactions.is_empty() {
            cli_io::print_info("No additional projections required within this window.");
            return;
        }

        cli_io::print_info("Upcoming projections:");
        for item in report.forecast.transactions.iter().take(8) {
            let status = self.scheduled_status_label(item.status);
            let route = self.describe_transaction_route(ledger, &item.transaction);
            let category = item
                .transaction
                .category_id
                .and_then(|id| self.lookup_category_name(ledger, id))
                .unwrap_or_else(|| "Uncategorized".into());
            let txn_currency = ledger.transaction_currency(&item.transaction);
            let amount = format_currency_value(
                item.transaction.budgeted_amount,
                &txn_currency,
                &ledger.locale,
                &ledger.format,
            );
            cli_io::print_info(format!(
                "  {date} | {amount} | {status:<8} | {route} ({category})",
                date = self.format_date(ledger, item.transaction.scheduled_date),
                amount = amount,
                status = status,
                route = route,
                category = category
            ));
        }
        if report.forecast.transactions.len() > 8 {
            cli_io::print_info(format!(
                "  ... {} additional projections",
                report.forecast.transactions.len() - 8
            ));
        }
    }

    fn scheduled_status_label(&self, status: ScheduledStatus) -> &'static str {
        match status {
            ScheduledStatus::Overdue => "Overdue",
            ScheduledStatus::Pending => "Pending",
            ScheduledStatus::Future => "Future",
        }
    }

    fn describe_transaction_route(&self, ledger: &Ledger, txn: &Transaction) -> String {
        let from = ledger
            .account(txn.from_account)
            .map(|acct| acct.name.clone())
            .unwrap_or_else(|| "Unknown".into());
        let to = ledger
            .account(txn.to_account)
            .map(|acct| acct.name.clone())
            .unwrap_or_else(|| "Unknown".into());
        format!("{} → {}", from, to)
    }

    fn transaction_summary_line(&self, ledger: &Ledger, txn: &Transaction) -> String {
        let category = txn
            .category_id
            .and_then(|id| self.lookup_category_name(ledger, id))
            .unwrap_or_else(|| "Uncategorized".into());
        let amount = format_currency_value(
            txn.budgeted_amount,
            &ledger.transaction_currency(txn),
            &ledger.locale,
            &ledger.format,
        );
        let route = self.describe_transaction_route(ledger, txn);
        let date = self.format_date(ledger, txn.scheduled_date);
        format!("{} {} ({} ) on {}", category, amount, route, date)
    }

    fn lookup_category_name(&self, ledger: &Ledger, id: Uuid) -> Option<String> {
        ledger
            .categories
            .iter()
            .find(|cat| cat.id == id)
            .map(|cat| cat.name.clone())
    }

    fn transaction_recurrence_hint(&self, txn: &Transaction) -> Option<String> {
        let rule = txn.recurrence.as_ref()?;
        let mut parts = vec![String::from("[recurring]"), rule.interval.label()];
        match rule.status {
            RecurrenceStatus::Active => parts.push("active".into()),
            RecurrenceStatus::Paused => parts.push("paused".into()),
            RecurrenceStatus::Completed => parts.push("completed".into()),
        }
        if let Some(next) = rule.next_scheduled {
            parts.push(format!("next {}", next));
        }
        if let Some(last) = rule.last_completed {
            parts.push(format!("last actual {}", last));
        }
        if rule.generated_occurrences > 0 {
            parts.push(format!("occurrences {}", rule.generated_occurrences));
        }
        Some(parts.join(" | "))
    }

    pub(crate) fn list_recurrences(&self, filter: RecurrenceListFilter) -> CommandResult {
        let had_entries = self.with_ledger(|ledger| {
            let today = Utc::now().date_naive();
            let snapshot_map: HashMap<Uuid, RecurrenceSnapshot> = ledger
                .recurrence_snapshots(today)
                .into_iter()
                .map(|snap| (snap.series_id, snap))
                .collect();
            if snapshot_map.is_empty() {
                cli_io::print_warning("No recurring schedules defined.");
                return Ok(false);
            }
            let mut entries: Vec<(usize, &Transaction, &RecurrenceSnapshot)> = ledger
                .transactions
                .iter()
                .enumerate()
                .filter_map(|(idx, txn)| {
                    txn.recurrence.as_ref().and_then(|recurrence| {
                        snapshot_map
                            .get(&recurrence.series_id)
                            .map(|snap| (idx, txn, snap))
                    })
                })
                .collect();
            entries.sort_by(|(_, _, a), (_, _, b)| match (a.next_due, b.next_due) {
                (Some(lhs), Some(rhs)) => lhs.cmp(&rhs),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            });

            Formatter::new().print_header("Recurring schedules");
            let mut shown = 0;
            for (index, txn, snapshot) in entries {
                if !filter.matches(snapshot) {
                    continue;
                }
                shown += 1;
                self.print_recurrence_entry(ledger, index, txn, snapshot);
            }
            if shown == 0 {
                cli_io::print_info("No recurring entries match the requested filter.");
            }
            Ok(shown > 0)
        })?;
        if had_entries {
            self.await_menu_escape()?;
        }
        Ok(())
    }

    fn print_recurrence_entry(
        &self,
        ledger: &Ledger,
        index: usize,
        txn: &Transaction,
        snapshot: &RecurrenceSnapshot,
    ) {
        let route = self.describe_transaction_route(ledger, txn);
        let category = txn
            .category_id
            .and_then(|id| self.lookup_category_name(ledger, id))
            .unwrap_or_else(|| "Uncategorized".into());
        let next_due = snapshot
            .next_due
            .map(|d| d.to_string())
            .unwrap_or_else(|| "None".into());
        let status = self.recurrence_status_label(&snapshot.status);
        cli_io::print_info(format!(
            "[{idx:>3}] {route} | {cat} | every {freq} | next {next} | overdue {overdue} | pending {pending}",
            idx = index,
            route = route,
            cat = category,
            freq = snapshot.interval_label,
            next = next_due,
            overdue = snapshot.overdue,
            pending = snapshot.pending
        ));
        cli_io::print_info(format!(
            "      amount {:.2} | status {status} | since {}",
            txn.budgeted_amount, snapshot.start_date
        ));
    }

    fn recurrence_status_label(&self, status: &RecurrenceStatus) -> &'static str {
        match status {
            RecurrenceStatus::Active => "Active",
            RecurrenceStatus::Paused => "Paused",
            RecurrenceStatus::Completed => "Completed",
        }
    }

    pub(crate) fn recurrence_edit(&mut self, index: usize) -> CommandResult {
        self.ensure_base_mode("Recurrence editing")?;
        let (scheduled_date, existing) = self.with_ledger(|ledger| {
            let txn = ledger.transactions.get(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            Ok((txn.scheduled_date, txn.recurrence.clone()))
        })?;
        let recurrence = self.prompt_recurrence(scheduled_date, existing.as_ref())?;
        self.with_ledger_mut(|ledger| {
            let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            txn.set_recurrence(Some(recurrence));
            ledger.refresh_recurrence_metadata();
            ledger.touch();
            Ok(())
        })?;
        cli_io::print_success(format!("Recurrence updated for transaction {}.", index));
        Ok(())
    }

    pub(crate) fn recurrence_clear(&mut self, index: usize) -> CommandResult {
        self.ensure_base_mode("Recurrence removal")?;
        let mut removed = false;
        self.with_ledger_mut(|ledger| {
            let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            if txn.recurrence.is_none() {
                cli_io::print_warning("Transaction has no recurrence defined.");
                return Ok(());
            }
            removed = true;
            txn.set_recurrence(None);
            txn.recurrence_series_id = None;
            ledger.refresh_recurrence_metadata();
            ledger.touch();
            Ok(())
        })?;
        if !removed {
            return Ok(());
        }
        cli_io::print_success(format!("Recurrence removed from transaction {}.", index));
        Ok(())
    }

    pub(crate) fn recurrence_set_status(
        &mut self,
        index: usize,
        status: RecurrenceStatus,
    ) -> CommandResult {
        self.ensure_base_mode("Recurrence status change")?;
        self.with_ledger_mut(|ledger| {
            let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            let recurrence = txn.recurrence.as_mut().ok_or_else(|| {
                CommandError::InvalidArguments("transaction has no recurrence".into())
            })?;
            recurrence.status = status.clone();
            ledger.refresh_recurrence_metadata();
            ledger.touch();
            Ok(())
        })?;
        cli_io::print_success(format!(
            "Recurrence status set to {:?} for transaction {}.",
            status, index
        ));
        Ok(())
    }

    pub(crate) fn recurrence_skip_date(&mut self, index: usize, date: NaiveDate) -> CommandResult {
        self.ensure_base_mode("Recurrence exception editing")?;
        let mut skipped = false;
        self.with_ledger_mut(|ledger| {
            let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            let recurrence = txn.recurrence.as_mut().ok_or_else(|| {
                CommandError::InvalidArguments("transaction has no recurrence".into())
            })?;
            if recurrence.exceptions.contains(&date) {
                cli_io::print_info(format!(
                    "Date {} already marked as skipped for this recurrence.",
                    date
                ));
                return Ok(());
            }
            skipped = true;
            recurrence.exceptions.push(date);
            recurrence.exceptions.sort();
            ledger.refresh_recurrence_metadata();
            ledger.touch();
            Ok(())
        })?;
        if !skipped {
            return Ok(());
        }
        cli_io::print_success(format!(
            "Added skip date {} for transaction {}.",
            date, index
        ));
        Ok(())
    }

    pub(crate) fn recurrence_sync(&mut self, reference: NaiveDate) -> CommandResult {
        self.ensure_base_mode("Recurrence synchronization")?;
        let created =
            self.with_ledger_mut(|ledger| Ok(ledger.materialize_due_recurrences(reference)))?;
        if created == 0 {
            cli_io::print_info("All due recurring instances already exist.");
        } else {
            cli_io::print_success(format!(
                "Created {} pending transactions from schedules.",
                created
            ));
        }
        Ok(())
    }

    pub(crate) fn resolve_simulation_name(
        &self,
        arg: Option<&str>,
        prompt: &str,
        fallback_active: bool,
        usage: &str,
    ) -> Result<Option<String>, CommandError> {
        let mut attempted_prompt = false;
        let candidate = match arg {
            Some(name) => Some(name.to_string()),
            None => {
                if fallback_active {
                    match self.active_simulation_name() {
                        Some(active) => Some(active.to_string()),
                        None => {
                            if self.can_prompt() {
                                attempted_prompt = true;
                                self.select_simulation_name(prompt)?
                            } else {
                                return Err(CommandError::InvalidArguments(
                                    "No active simulation. Use `enter-simulation <name>` first."
                                        .into(),
                                ));
                            }
                        }
                    }
                } else if self.can_prompt() {
                    attempted_prompt = true;
                    self.select_simulation_name(prompt)?
                } else {
                    None
                }
            }
        };

        match candidate {
            Some(name) => {
                let validated = self.with_ledger(|ledger| {
                    if ledger.simulation(&name).is_none() {
                        Err(CommandError::InvalidArguments(format!(
                            "simulation `{}` not found",
                            name
                        )))
                    } else {
                        Ok(name)
                    }
                })?;
                Ok(Some(validated))
            }
            None => {
                if attempted_prompt || self.can_prompt() {
                    Ok(None)
                } else {
                    Err(CommandError::InvalidArguments(usage.into()))
                }
            }
        }
    }

    pub(crate) fn print_simulation_changes(&self, sim_name: &str) -> CommandResult {
        self.with_ledger(|ledger| {
            let sim = ledger.simulation(sim_name).ok_or_else(|| {
                CommandError::InvalidArguments(format!("simulation `{}` not found", sim_name))
            })?;
            cli_io::print_info(format!("Simulation `{}` ({:?})", sim.name, sim.status));
            if sim.changes.is_empty() {
                cli_io::print_info("No pending changes.");
            } else {
                for (idx, change) in sim.changes.iter().enumerate() {
                    match change {
                        SimulationChange::AddTransaction { transaction } => {
                            cli_io::print_info(format!(
                                "  [{:>2}] Add transaction {} -> {} on {} (budgeted {:.2})",
                                idx,
                                transaction.from_account,
                                transaction.to_account,
                                transaction.scheduled_date,
                                transaction.budgeted_amount
                            ))
                        }
                        SimulationChange::ModifyTransaction(patch) => cli_io::print_info(format!(
                            "  [{:>2}] Modify transaction {}",
                            idx, patch.transaction_id
                        )),
                        SimulationChange::ExcludeTransaction { transaction_id } => {
                            cli_io::print_info(format!(
                                "  [{:>2}] Exclude transaction {}",
                                idx, transaction_id
                            ))
                        }
                    }
                }
            }
            Ok(())
        })
    }

    pub(crate) fn simulation_add_transaction(&mut self, sim_name: &str) -> CommandResult {
        self.run_transaction_add_wizard(Some(sim_name))
    }

    pub(crate) fn simulation_exclude_transaction(&mut self, sim_name: &str) -> CommandResult {
        let txn_id = self.select_transaction_id("Exclude which transaction?")?;
        self.with_ledger_mut(|ledger| {
            ledger
                .exclude_transaction_in_simulation(sim_name, txn_id)
                .map_err(CommandError::from_core)
        })?;
        cli_io::print_success(format!("Transaction {} excluded in `{}`", txn_id, sim_name));
        Ok(())
    }

    pub(crate) fn simulation_modify_transaction(&mut self, sim_name: &str) -> CommandResult {
        let txn_id = self.select_transaction_id("Modify which transaction?")?;

        let budgeted_input =
            self.prompt_optional_f64("New budgeted amount (leave blank to keep)")?;
        let actual_input = self
            .prompt_optional_f64_or_clear("New actual amount (blank to keep, 'none' to clear)")?;
        let scheduled_input =
            self.prompt_optional_date("New scheduled date (YYYY-MM-DD, blank to keep)")?;
        let actual_date_input = self.prompt_optional_date_or_clear(
            "New actual date (YYYY-MM-DD, blank to keep, 'none' to clear)",
        )?;

        let patch = SimulationTransactionPatch {
            transaction_id: txn_id,
            from_account: None,
            to_account: None,
            category_id: None,
            scheduled_date: scheduled_input,
            actual_date: actual_date_input,
            budgeted_amount: budgeted_input,
            actual_amount: actual_input,
        };

        if !patch.has_effect() {
            return Err(CommandError::InvalidArguments(
                "No changes were specified".into(),
            ));
        }

        self.with_ledger_mut(|ledger| {
            ledger
                .modify_transaction_in_simulation(sim_name, patch)
                .map_err(CommandError::from_core)
        })?;
        cli_io::print_success(format!("Transaction {} modified in `{}`", txn_id, sim_name));
        Ok(())
    }

    fn select_transaction_id(&self, prompt: &str) -> Result<Uuid, CommandError> {
        let items = self.with_ledger(|ledger| {
            if ledger.transactions.is_empty() {
                return Err(CommandError::InvalidArguments(
                    "No transactions available".into(),
                ));
            }
            let items: Vec<String> = ledger
                .transactions
                .iter()
                .enumerate()
                .map(|(idx, txn)| {
                    format!(
                        "[{}] {} -> {} | {} | {:.2}",
                        idx,
                        txn.from_account,
                        txn.to_account,
                        txn.scheduled_date,
                        txn.budgeted_amount
                    )
                })
                .collect();
            Ok(items)
        })?;
        let selection = Select::with_theme(&self.theme)
            .with_prompt(prompt)
            .items(&items)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        self.with_ledger(|ledger| {
            ledger
                .transactions
                .get(selection)
                .map(|txn| txn.id)
                .ok_or_else(|| {
                    CommandError::InvalidArguments("transaction index out of range".into())
                })
        })
    }

    fn prompt_optional_f64(&self, prompt: &str) -> Result<Option<f64>, CommandError> {
        let input: String = Input::with_theme(&self.theme)
            .with_prompt(prompt)
            .interact_text()
            .map_err(CommandError::from)?;
        if input.trim().is_empty() {
            Ok(None)
        } else {
            input
                .trim()
                .parse::<f64>()
                .map(Some)
                .map_err(|_| CommandError::InvalidArguments("Invalid number supplied".into()))
        }
    }

    fn prompt_optional_f64_or_clear(
        &self,
        prompt: &str,
    ) -> Result<Option<Option<f64>>, CommandError> {
        let input: String = Input::with_theme(&self.theme)
            .with_prompt(prompt)
            .interact_text()
            .map_err(CommandError::from)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else if trimmed.eq_ignore_ascii_case("none") {
            Ok(Some(None))
        } else {
            let value = trimmed
                .parse::<f64>()
                .map_err(|_| CommandError::InvalidArguments("Invalid number supplied".into()))?;
            Ok(Some(Some(value)))
        }
    }

    fn prompt_optional_date(&self, prompt: &str) -> Result<Option<NaiveDate>, CommandError> {
        let input: String = Input::with_theme(&self.theme)
            .with_prompt(prompt)
            .interact_text()
            .map_err(CommandError::from)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
                .map(Some)
                .map_err(|_| CommandError::InvalidArguments("Invalid date format".into()))
        }
    }

    fn prompt_optional_date_or_clear(
        &self,
        prompt: &str,
    ) -> Result<Option<Option<NaiveDate>>, CommandError> {
        let input: String = Input::with_theme(&self.theme)
            .with_prompt(prompt)
            .interact_text()
            .map_err(CommandError::from)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else if trimmed.eq_ignore_ascii_case("none") {
            Ok(Some(None))
        } else {
            NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
                .map(|date| Some(Some(date)))
                .map_err(|_| CommandError::InvalidArguments("Invalid date format".into()))
        }
    }
}

fn selection_override_from_env() -> Option<SelectionOverride> {
    let raw = env::var("BUFY_TEST_SELECTIONS").ok()?;
    let override_data = SelectionOverride::default();
    for token in raw
        .split('|')
        .map(|segment| segment.trim())
        .filter(|s| !s.is_empty())
    {
        let choice = match token.to_ascii_uppercase().as_str() {
            "CANCEL" | "<ESC>" => None,
            value => Some(
                value
                    .parse::<usize>()
                    .unwrap_or_else(|_| panic!("Invalid BUFY selection token `{value}`")),
            ),
        };
        override_data.push(choice);
    }
    Some(override_data)
}

fn parse_period(input: &str) -> Result<BudgetPeriod, CommandError> {
    Ok(BudgetPeriod(parse_time_interval_str(input)?))
}

fn interval_options() -> &'static [&'static str] {
    &["Monthly", "Weekly", "Daily", "Yearly", "Custom..."]
}

fn parse_time_interval_str(input: &str) -> Result<TimeInterval, CommandError> {
    let normalized = input.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(CommandError::InvalidArguments(
            "interval description cannot be empty".into(),
        ));
    }

    let direct = match normalized.as_str() {
        "daily" => Some(TimeInterval {
            every: 1,
            unit: TimeUnit::Day,
        }),
        "weekly" => Some(TimeInterval {
            every: 1,
            unit: TimeUnit::Week,
        }),
        "monthly" => Some(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        }),
        "yearly" => Some(TimeInterval {
            every: 1,
            unit: TimeUnit::Year,
        }),
        _ => None,
    };
    if let Some(interval) = direct {
        return Ok(interval);
    }

    let cleaned = normalized.replace(['-', '_'], " ");
    let mut parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.first().copied() == Some("every") {
        parts.remove(0);
    }

    let (number_str, unit_str) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else if parts.len() == 1 {
        split_numeric_unit(parts[0]).ok_or_else(|| {
            CommandError::InvalidArguments(format!("unable to parse interval `{}`", input))
        })?
    } else {
        return Err(CommandError::InvalidArguments(format!(
            "unable to parse interval `{}`",
            input
        )));
    };

    let every: u32 = number_str.parse().map_err(|_| {
        CommandError::InvalidArguments(format!("invalid interval count `{}`", number_str))
    })?;
    if every == 0 {
        return Err(CommandError::InvalidArguments(
            "interval count must be greater than zero".into(),
        ));
    }

    let unit = parse_time_unit(unit_str)?;
    Ok(TimeInterval { every, unit })
}

fn parse_category_budget_period_str(input: &str) -> Result<CategoryBudgetPeriod, CommandError> {
    let normalized = input.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(CommandError::InvalidArguments(
            "budget period cannot be empty".into(),
        ));
    }
    match normalized.as_str() {
        "daily" => return Ok(CategoryBudgetPeriod::Daily),
        "weekly" => return Ok(CategoryBudgetPeriod::Weekly),
        "monthly" => return Ok(CategoryBudgetPeriod::Monthly),
        "yearly" => return Ok(CategoryBudgetPeriod::Yearly),
        _ => {}
    }
    if normalized.starts_with("custom") {
        let value_part = normalized
            .split(&[':', '=', ' '][..])
            .skip(1)
            .find(|segment| !segment.is_empty())
            .ok_or_else(|| {
                CommandError::InvalidArguments(
                    "custom period must include a numeric day count (e.g. custom:45)".into(),
                )
            })?;
        let days: u32 = value_part.parse().map_err(|_| {
            CommandError::InvalidArguments(format!(
                "invalid custom period `{}` (use custom:<days>)",
                input
            ))
        })?;
        if days == 0 {
            return Err(CommandError::InvalidArguments(
                "custom period must be greater than 0 days".into(),
            ));
        }
        return Ok(CategoryBudgetPeriod::Custom(days));
    }
    Err(CommandError::InvalidArguments(format!(
        "unknown budget period `{}` (use daily, weekly, monthly, yearly, or custom:<days>)",
        input
    )))
}

fn category_budget_period_token(period: &CategoryBudgetPeriod) -> String {
    match period {
        CategoryBudgetPeriod::Daily => "daily".into(),
        CategoryBudgetPeriod::Weekly => "weekly".into(),
        CategoryBudgetPeriod::Monthly => "monthly".into(),
        CategoryBudgetPeriod::Yearly => "yearly".into(),
        CategoryBudgetPeriod::Custom(days) => format!("custom:{days}"),
    }
}

fn describe_category_budget_period(period: &CategoryBudgetPeriod) -> String {
    match period {
        CategoryBudgetPeriod::Daily => "Daily".into(),
        CategoryBudgetPeriod::Weekly => "Weekly".into(),
        CategoryBudgetPeriod::Monthly => "Monthly".into(),
        CategoryBudgetPeriod::Yearly => "Yearly".into(),
        CategoryBudgetPeriod::Custom(days) => {
            format!("Every {} day{}", days, if *days == 1 { "" } else { "s" })
        }
    }
}

fn parse_budget_amount(value: &str) -> Result<f64, CommandError> {
    let amount: f64 = value
        .parse()
        .map_err(|_| CommandError::InvalidArguments(format!("invalid amount `{}`", value)))?;
    if amount <= 0.0 {
        return Err(CommandError::InvalidArguments(
            "amount must be greater than 0".into(),
        ));
    }
    Ok(amount)
}

fn split_period_flag(args: &[&str]) -> (Vec<String>, Option<String>) {
    let mut positionals = Vec::new();
    let mut period = None;
    let mut idx = 0;
    while idx < args.len() {
        let token = args[idx];
        let lowered = token.to_lowercase();
        if lowered.starts_with("--period=") {
            if let Some(eq_index) = token.find('=') {
                period = Some(token[eq_index + 1..].to_string());
            } else {
                period = Some(String::new());
            }
        } else if lowered == "--period" {
            if idx + 1 < args.len() {
                period = Some(args[idx + 1].to_string());
                idx += 1;
            } else {
                period = Some(String::new());
            }
        } else {
            positionals.push(token.to_string());
        }
        idx += 1;
    }
    (positionals, period)
}

fn format_backup_label(file_name: &str) -> String {
    let trimmed = file_name.trim_end_matches(".json");
    let segments: Vec<&str> = trimmed.split('_').collect();
    if segments.len() < 3 {
        return file_name.to_string();
    }
    let date_part = segments[segments.len() - 2];
    let time_part = segments[segments.len() - 1];
    let is_well_formed = date_part.len() == 8
        && time_part.len() == 4
        && date_part.chars().all(|c| c.is_ascii_digit())
        && time_part.chars().all(|c| c.is_ascii_digit());
    if !is_well_formed {
        return file_name.to_string();
    }
    let raw = format!("{}{}", date_part, time_part);
    let timestamp = NaiveDateTime::parse_from_str(&raw, "%Y%m%d%H%M")
        .ok()
        .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
    if let Some(utc) = timestamp {
        let local = utc.with_timezone(&Local);
        format!(
            "{} (Created: {})",
            file_name,
            local.format("%Y-%m-%d %H:%M")
        )
    } else {
        file_name.to_string()
    }
}

fn parse_account_kind(input: &str) -> Result<AccountKind, CommandError> {
    match input.to_lowercase().as_str() {
        "bank" => Ok(AccountKind::Bank),
        "cash" => Ok(AccountKind::Cash),
        "savings" => Ok(AccountKind::Savings),
        "expensedestination" | "expense" => Ok(AccountKind::ExpenseDestination),
        "incomesource" | "income" => Ok(AccountKind::IncomeSource),
        "unknown" => Ok(AccountKind::Unknown),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown account kind `{}`",
            other
        ))),
    }
}

fn parse_category_kind(input: &str) -> Result<CategoryKind, CommandError> {
    match input.to_lowercase().as_str() {
        "expense" => Ok(CategoryKind::Expense),
        "income" => Ok(CategoryKind::Income),
        "transfer" => Ok(CategoryKind::Transfer),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown category kind `{}`",
            other
        ))),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RecurrenceListFilter {
    All,
    Pending,
    Overdue,
    Active,
    Paused,
    Completed,
}

impl RecurrenceListFilter {
    pub(crate) fn parse(token: Option<&str>) -> Result<Self, CommandError> {
        match token.map(|t| t.to_lowercase()) {
            None => Ok(RecurrenceListFilter::All),
            Some(ref value) if value == "all" => Ok(RecurrenceListFilter::All),
            Some(ref value) if value == "pending" => Ok(RecurrenceListFilter::Pending),
            Some(ref value) if value == "overdue" => Ok(RecurrenceListFilter::Overdue),
            Some(ref value) if value == "active" => Ok(RecurrenceListFilter::Active),
            Some(ref value) if value == "paused" => Ok(RecurrenceListFilter::Paused),
            Some(ref value) if value == "completed" => Ok(RecurrenceListFilter::Completed),
            Some(value) => Err(CommandError::InvalidArguments(format!(
                "unknown recurrence filter `{}`",
                value
            ))),
        }
    }

    fn matches(&self, snapshot: &RecurrenceSnapshot) -> bool {
        match self {
            RecurrenceListFilter::All => true,
            RecurrenceListFilter::Pending => snapshot.pending > 0,
            RecurrenceListFilter::Overdue => snapshot.overdue > 0,
            RecurrenceListFilter::Active => matches!(snapshot.status, RecurrenceStatus::Active),
            RecurrenceListFilter::Paused => matches!(snapshot.status, RecurrenceStatus::Paused),
            RecurrenceListFilter::Completed => {
                matches!(snapshot.status, RecurrenceStatus::Completed)
            }
        }
    }
}

fn split_numeric_unit(token: &str) -> Option<(&str, &str)> {
    let pos = token.find(|c: char| !c.is_ascii_digit())?;
    let (number, rest) = token.split_at(pos);
    if number.is_empty() || rest.is_empty() {
        None
    } else {
        Some((number, rest))
    }
}

fn parse_time_unit(token: &str) -> Result<TimeUnit, CommandError> {
    match token.trim_matches('s') {
        "day" | "d" => Ok(TimeUnit::Day),
        "week" | "w" => Ok(TimeUnit::Week),
        "month" | "mo" | "m" => Ok(TimeUnit::Month),
        "year" | "yr" | "y" => Ok(TimeUnit::Year),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown time unit `{}`",
            other
        ))),
    }
}

fn parse_positive_or_default(arg: Option<&&str>, default: usize) -> Result<usize, CommandError> {
    if let Some(value) = arg {
        let parsed = value.parse::<usize>().map_err(|_| {
            CommandError::InvalidArguments("offset must be a positive integer".into())
        })?;
        if parsed == 0 {
            Err(CommandError::InvalidArguments(
                "offset must be greater than zero".into(),
            ))
        } else {
            Ok(parsed)
        }
    } else {
        Ok(default)
    }
}

pub(crate) fn parse_date(input: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(input, "%Y-%m-%d").map_err(|_| {
        CommandError::InvalidArguments(format!("invalid date `{}` (use YYYY-MM-DD)", input))
    })
}

fn short_id(id: Uuid) -> String {
    let mut short = id.simple().to_string();
    short.truncate(8);
    short
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Ledger not loaded. Use `ledger new` or `ledger load` first.")]
    LedgerNotLoaded,
    #[error("{0}")]
    InvalidArguments(String),
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Core(#[from] BudgetError),
    #[error(transparent)]
    Dialoguer(#[from] dialoguer::Error),
    #[error("exit requested")]
    ExitRequested,
}

impl From<ServiceError> for CommandError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Core(err) => CommandError::Core(err),
            ServiceError::Invalid(message) => CommandError::InvalidArguments(message),
        }
    }
}

impl CommandError {
    pub(crate) fn from_core(error: BudgetError) -> Self {
        CommandError::Core(error)
    }
}

impl From<ProviderError> for CommandError {
    fn from(err: ProviderError) -> Self {
        match err {
            ProviderError::MissingLedger => CommandError::LedgerNotLoaded,
            ProviderError::Store(message) => CommandError::Message(message),
        }
    }
}

impl From<CliError> for CommandError {
    fn from(err: CliError) -> Self {
        match err {
            CliError::Core(inner) => CommandError::Core(inner),
            CliError::Input(message) | CliError::Command(message) => {
                CommandError::InvalidArguments(message)
            }
        }
    }
}

impl From<CommandError> for CliError {
    fn from(err: CommandError) -> Self {
        CliError::Command(err.to_string())
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        CliError::Command(err.to_string())
    }
}

#[cfg(test)]
pub(crate) fn process_script(lines: &[&str]) -> Result<ShellContext, CliError> {
    let mut app = ShellContext::new(CliMode::Script)?;
    for line in lines {
        match app.process_line(line)? {
            LoopControl::Continue => {}
            LoopControl::Exit => break,
        }
    }
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::selection::providers::{
        AccountSelectionProvider, CategorySelectionProvider, ConfigBackupSelectionProvider,
        LedgerBackupSelectionProvider, TransactionSelectionProvider,
    };
    use crate::cli::selection::SelectionManager;
    use crate::cli::selectors::SelectionOutcome;
    use crate::config::{Config, ConfigManager};
    use crate::core::ledger_manager::LedgerManager;
    use crate::ledger::{AccountKind, CategoryKind, TimeInterval, TimeUnit};
    use crate::ledger::{Simulation, SimulationStatus};
    use crate::storage::json_backend::JsonStorage;
    use chrono::{NaiveDate, Utc};
    use std::sync::{Arc, RwLock};
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn parse_line_handles_quotes() {
        let tokens =
            crate::cli::shell::parse_command_line("ledger new \"Demo Ledger\" monthly").unwrap();
        assert_eq!(tokens, vec!["ledger", "new", "Demo Ledger", "monthly"]);
    }

    #[test]
    fn script_runner_creates_ledger() {
        let context = process_script(&["ledger new Demo 3 months", "exit"]).unwrap();
        context
            .with_ledger(|ledger| {
                assert_eq!(ledger.name, "Demo");
                assert_eq!(ledger.budget_period.0.every, 3);
                assert_eq!(ledger.budget_period.0.unit, TimeUnit::Month);
                Ok(())
            })
            .expect("ledger present");
    }

    #[test]
    fn script_can_save_and_load() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let setup_cmds: Vec<String> = vec![
            "ledger new Testing every 2 weeks".into(),
            format!("ledger save {}", path.display()),
            "exit".into(),
        ];
        let setup_refs: Vec<&str> = setup_cmds.iter().map(String::as_str).collect();
        process_script(&setup_refs).unwrap();

        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"Testing\""));

        let load_cmds: Vec<String> = vec![
            format!("ledger load {}", path.display()),
            "summary".into(),
            "exit".into(),
        ];
        let load_refs: Vec<&str> = load_cmds.iter().map(String::as_str).collect();
        let context = process_script(&load_refs).unwrap();
        context
            .with_ledger(|ledger| {
                assert_eq!(ledger.name, "Testing");
                assert_eq!(ledger.budget_period.0.every, 2);
                assert_eq!(ledger.budget_period.0.unit, TimeUnit::Week);
                Ok(())
            })
            .expect("ledger present");
    }

    #[test]
    fn parse_interval_accepts_every_keyword() {
        let interval = super::parse_time_interval_str("every 6 weeks").unwrap();
        assert_eq!(interval.every, 6);
        assert_eq!(interval.unit, TimeUnit::Week);
    }

    #[test]
    fn parse_interval_accepts_compact_form() {
        let interval = super::parse_time_interval_str("12months").unwrap();
        assert_eq!(interval.every, 12);
        assert_eq!(interval.unit, TimeUnit::Month);
    }

    #[test]
    fn parse_interval_rejects_zero() {
        let err = super::parse_time_interval_str("0 days").unwrap_err();
        matches!(err, CommandError::InvalidArguments(_));
    }

    fn sample_ledger() -> Ledger {
        let mut ledger = Ledger::new("Sample", BudgetPeriod::monthly());
        let bank = ledger.add_account(Account::new("Checking", AccountKind::Bank));
        let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));

        let category = Category::new("Household", CategoryKind::Expense);
        let category_id = category.id;
        ledger.add_category(category);

        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let mut txn = Transaction::new(bank, savings, Some(category_id), date, 100.0);
        let interval = TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        };
        let recurrence = Recurrence::new(date, interval, RecurrenceMode::FixedSchedule);
        txn.set_recurrence(Some(recurrence));
        ledger.add_transaction(txn);

        let now = Utc::now();
        ledger.simulations.push(Simulation {
            id: Uuid::new_v4(),
            name: "Scenario".into(),
            notes: None,
            status: SimulationStatus::Pending,
            created_at: now,
            updated_at: now,
            applied_at: None,
            changes: Vec::new(),
        });

        ledger
    }

    fn context_with_ledger() -> ShellContext {
        let mut context = ShellContext::new(CliMode::Script).unwrap();
        let ledger = sample_ledger();
        context.set_ledger(ledger, None, Some("sample".into()));
        context
    }

    #[test]
    fn account_selection_positive() {
        let context = context_with_ledger();
        let outcome = SelectionManager::new(AccountSelectionProvider::new(&context))
            .choose_with("Select account", "No accounts available.", |_, _| {
                Ok(Some(1))
            })
            .unwrap();
        match outcome {
            SelectionOutcome::Selected(id) => assert_eq!(id, 1),
            _ => panic!("expected account selection"),
        }
    }

    #[test]
    fn account_selection_cancelled() {
        let context = context_with_ledger();
        let outcome = SelectionManager::new(AccountSelectionProvider::new(&context))
            .choose_with("Select account", "No accounts available.", |_, _| Ok(None))
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn category_selection_paths() {
        let context = context_with_ledger();
        let outcome = SelectionManager::new(CategorySelectionProvider::new(&context))
            .choose_with("Select category", "No categories available.", |_, _| {
                Ok(Some(0))
            })
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Selected(0)));

        let outcome = SelectionManager::new(CategorySelectionProvider::new(&context))
            .choose_with("Select category", "No categories available.", |_, _| {
                Ok(None)
            })
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn transaction_selection_paths() {
        let context = context_with_ledger();
        let outcome = SelectionManager::new(TransactionSelectionProvider::new(&context))
            .choose_with(
                "Select transaction",
                "No transactions available.",
                |_, _| Ok(Some(0)),
            )
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Selected(0)));

        let outcome = SelectionManager::new(TransactionSelectionProvider::new(&context))
            .choose_with(
                "Select transaction",
                "No transactions available.",
                |_, _| Ok(None),
            )
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn simulation_selection_via_resolver() {
        let mut app = ShellContext::new(CliMode::Script).unwrap();
        let ledger = sample_ledger();
        app.set_ledger(ledger, None, Some("sample".into()));

        app.set_selection_choices([Some(0)]);
        let selected = app
            .resolve_simulation_name(None, "Select simulation", false, "usage")
            .unwrap();
        assert_eq!(selected.as_deref(), Some("Scenario"));

        app.reset_selection_choices();
        app.set_selection_choices([None]);
        let cancel = app
            .resolve_simulation_name(None, "Select simulation", false, "usage")
            .unwrap();
        assert!(cancel.is_none());
    }

    #[test]
    fn ledger_backup_selection_paths() {
        let temp = tempdir().unwrap();
        let storage = JsonStorage::new(Some(temp.path().to_path_buf()), Some(5)).unwrap();
        let mut context = ShellContext::new(CliMode::Script).unwrap();
        context.storage = storage.clone();
        context.ledger_manager = Arc::new(RwLock::new(LedgerManager::new(Box::new(storage))));
        context.set_ledger(sample_ledger(), None, Some("sample".into()));
        context.manager_mut().backup(None).unwrap();
        let expected = context
            .manager()
            .list_backups("sample")
            .unwrap()
            .first()
            .cloned()
            .expect("backup created");

        let outcome = SelectionManager::new(LedgerBackupSelectionProvider::new(&context))
            .choose_with("Select backup", "No backups available.", |_, _| Ok(Some(0)))
            .unwrap();
        match outcome {
            SelectionOutcome::Selected(name) => assert_eq!(name, expected),
            _ => panic!("expected backup selection"),
        }

        let outcome = SelectionManager::new(LedgerBackupSelectionProvider::new(&context))
            .choose_with("Select backup", "No backups available.", |_, _| Ok(None))
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn config_backup_selection_paths() {
        let temp = tempdir().unwrap();
        let manager = Arc::new(RwLock::new(
            ConfigManager::with_base_dir(temp.path().to_path_buf()).unwrap(),
        ));
        let mut config = Config::default();
        config.locale = "en-GB".into();
        {
            let manager_guard = manager.read().unwrap();
            manager_guard.save(&config).unwrap();
        }
        let backup_name = {
            let manager_guard = manager.read().unwrap();
            manager_guard
                .backup(&config, Some("baseline"))
                .expect("backup config")
        };

        let outcome = SelectionManager::new(ConfigBackupSelectionProvider::new(manager.clone()))
            .choose_with(
                "Select config",
                "No configuration backups found.",
                |_, _| Ok(Some(0)),
            )
            .unwrap();
        match outcome {
            SelectionOutcome::Selected(name) => assert_eq!(name, backup_name),
            _ => panic!("expected config selection"),
        }

        let outcome = SelectionManager::new(ConfigBackupSelectionProvider::new(manager))
            .choose_with(
                "Select config",
                "No configuration backups found.",
                |_, _| Ok(None),
            )
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }
}
