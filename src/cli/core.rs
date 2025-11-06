use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    io,
    path::{Path, PathBuf},
};

use chrono::{Duration, Local, NaiveDate, Utc};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rustyline::error::ReadlineError;
use strsim::levenshtein;
use uuid::Uuid;

use crate::{
    currency::{format_currency_value, format_date},
    errors::LedgerError,
    ledger::{
        account::AccountKind, category::CategoryKind, Account, BudgetPeriod, BudgetScope,
        BudgetSummary, Category, DateWindow, ForecastReport, Ledger, Recurrence, RecurrenceEnd,
        RecurrenceMode, RecurrenceSnapshot, RecurrenceStatus, ScheduledStatus,
        SimulationBudgetImpact, SimulationChange, SimulationTransactionPatch, TimeInterval,
        TimeUnit, Transaction, TransactionStatus,
    },
    utils::persistence::{ConfigData, ConfigSnapshot, LedgerStore},
};

use crate::cli::forms::{
    AccountFormData, AccountInitialData, AccountWizard, CategoryFormData, CategoryInitialData,
    CategoryWizard, DialoguerInteraction, FormEngine, FormResult, TransactionFormData,
    TransactionInitialData, TransactionRecurrenceAction, TransactionWizard,
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

use super::commands::{self, CommandDefinition, CommandRegistry};
use super::output::{
    error as output_error, info as output_info, section as output_section,
    success as output_success, warning as output_warning,
};
use super::state::CliState;

const PROMPT_ARROW: &str = "⮞";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    Interactive,
    Script,
}

#[derive(Default)]
struct SelectionOverride {
    queue: RefCell<VecDeque<Option<usize>>>,
}

impl SelectionOverride {
    #[cfg(test)]
    fn push(&self, choice: Option<usize>) {
        self.queue.borrow_mut().push_back(choice);
    }

    fn pop(&self) -> Option<Option<usize>> {
        self.queue.borrow_mut().pop_front()
    }

    fn has_choices(&self) -> bool {
        !self.queue.borrow().is_empty()
    }

    #[cfg(test)]
    fn clear(&self) {
        self.queue.borrow_mut().clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoopControl {
    Continue,
    Exit,
}

pub type CommandResult = Result<(), CommandError>;

pub struct ShellContext {
    mode: CliMode,
    registry: CommandRegistry,
    state: CliState,
    theme: ColorfulTheme,
    store: LedgerStore,
    selection_override: Option<SelectionOverride>,
}

impl ShellContext {
    fn auto_load_last(&mut self) -> Result<(), CliError> {
        if self.mode != CliMode::Interactive {
            return Ok(());
        }
        if self.state.ledger.is_some() {
            return Ok(());
        }
        let last = match self.store.last_ledger() {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!("last ledger lookup failed: {err}");
                return Ok(());
            }
        };
        let Some(name) = last else {
            return Ok(());
        };
        if let Ok(report) = self.store.load_named(&name) {
            let path = self.store.ledger_path(&name);
            self.state
                .set_ledger(report.ledger, Some(path), Some(name.clone()));
            self.report_load(&report.warnings, &report.migrations);
            output_success(format!("Automatically loaded last ledger `{}`.", name));
        }
        Ok(())
    }

    pub fn new(mode: CliMode) -> Result<Self, CliError> {
        let registry = CommandRegistry::new(commands::all_definitions());

        let store =
            LedgerStore::new_default().map_err(|err| CliError::Internal(err.to_string()))?;

        let mut app = Self {
            mode,
            registry,
            state: CliState::new(),
            theme: ColorfulTheme::default(),
            store,
            selection_override: None,
        };

        app.auto_load_last()?;
        Ok(app)
    }

    pub(crate) fn mode(&self) -> CliMode {
        self.mode
    }

    pub(crate) fn theme(&self) -> &ColorfulTheme {
        &self.theme
    }

    pub(crate) fn ledger_name(&self) -> Option<&str> {
        self.state.ledger_name()
    }

    pub(crate) fn ledger_path(&self) -> Option<PathBuf> {
        self.state.ledger_path.clone()
    }

    pub(crate) fn set_active_simulation(&mut self, name: Option<String>) {
        self.state.set_active_simulation(name);
    }

    pub(crate) fn clear_active_simulation(&mut self) {
        self.state.set_active_simulation(None);
    }

    fn ensure_base_mode(&self, action: &str) -> Result<(), CommandError> {
        if self.state.active_simulation().is_some() {
            Err(CommandError::InvalidArguments(format!(
                "{} is unavailable while editing a simulation. Use `leave-simulation` first.",
                action
            )))
        } else {
            Ok(())
        }
    }

    fn format_amount(&self, ledger: &Ledger, amount: f64) -> String {
        format_currency_value(
            amount,
            ledger.base_currency(),
            &ledger.locale,
            &ledger.format,
        )
    }

    fn format_date(&self, ledger: &Ledger, date: NaiveDate) -> String {
        format_date(&ledger.locale, date)
    }

    pub(crate) fn show_config(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        output_section("Configuration");
        output_info(format!(
            "  Base currency: {}",
            ledger.base_currency.as_str()
        ));
        output_info(format!("  Locale: {}", ledger.locale.language_tag));
        output_info(format!(
            "  Negative style: {:?}",
            ledger.format.negative_style
        ));
        output_info(format!(
            "  Screen reader mode: {}",
            if ledger.format.screen_reader_mode {
                "on"
            } else {
                "off"
            }
        ));
        output_info(format!(
            "  High contrast mode: {}",
            if ledger.format.high_contrast_mode {
                "on"
            } else {
                "off"
            }
        ));
        output_info(format!("  Valuation policy: {:?}", ledger.valuation_policy));
        Ok(())
    }

    pub(crate) fn require_named_ledger(&self) -> Result<&str, CommandError> {
        self.state.ledger_name().ok_or_else(|| {
            CommandError::InvalidArguments(
                "No named ledger associated. Use `save-ledger <name>` once to bind it.".into(),
            )
        })
    }

    pub(crate) fn prompt(&self) -> String {
        let context = self
            .state
            .ledger
            .as_ref()
            .map(|ledger| format!("ledger({})", ledger.name))
            .unwrap_or_else(|| "no-ledger".to_string());

        let sim_segment = self
            .state
            .active_simulation()
            .map(|name| format!(" [sim:{}]", name))
            .unwrap_or_default();
        format!(
            "{context}{sim_segment} {arrow} ",
            context = context,
            sim_segment = sim_segment,
            arrow = PROMPT_ARROW
        )
    }

    fn report_load(&self, warnings: &[String], migrations: &[String]) {
        for note in migrations {
            output_info(format!("Migration: {}", note));
        }
        for warning in warnings {
            output_warning(warning);
        }
    }

    pub(crate) fn dispatch(
        &mut self,
        command: &str,
        raw: &str,
        args: &[&str],
    ) -> Result<LoopControl, CommandError> {
        if let Some(definition) = self.registry.get(command) {
            match (definition.handler)(self, args) {
                Ok(()) => Ok(LoopControl::Continue),
                Err(CommandError::ExitRequested) => Ok(LoopControl::Exit),
                Err(err) => Err(err),
            }
        } else {
            self.suggest_command(raw);
            Ok(LoopControl::Continue)
        }
    }

    pub(crate) fn suggest_command(&self, input: &str) {
        output_warning(format!(
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
                output_info(format!("Suggestion: `{}`?", best));
            }
        }
    }

    pub(crate) fn confirm_exit(&self) -> Result<bool, CliError> {
        if self.mode == CliMode::Script {
            return Ok(true);
        }

        let confirm = Confirm::with_theme(&self.theme)
            .with_prompt("Exit shell?")
            .default(false)
            .interact()
            .map_err(CommandError::from)?;

        Ok(confirm)
    }

    pub(crate) fn report_error(&self, err: CommandError) -> Result<(), CliError> {
        match err {
            CommandError::ExitRequested => Ok(()),
            other => {
                self.print_error(&other.to_string());
                Ok(())
            }
        }
    }

    pub(crate) fn print_error(&self, message: &str) {
        output_error(message);
    }

    pub(crate) fn print_warning(&self, message: &str) {
        output_warning(message);
    }

    pub(crate) fn current_ledger(&self) -> Result<&Ledger, CommandError> {
        self.state
            .ledger
            .as_ref()
            .ok_or(CommandError::LedgerNotLoaded)
    }

    pub(crate) fn current_ledger_mut(&mut self) -> Result<&mut Ledger, CommandError> {
        self.state
            .ledger
            .as_mut()
            .ok_or(CommandError::LedgerNotLoaded)
    }

    pub(crate) fn active_simulation_name(&self) -> Option<&str> {
        self.state.active_simulation()
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
            TransactionSelectionProvider::new(&self.state),
            prompt,
            "No transactions available.",
        )
    }

    fn select_simulation_name(&self, prompt: &str) -> Result<Option<String>, CommandError> {
        self.select_with(
            SimulationSelectionProvider::new(&self.state),
            prompt,
            "No saved simulations available.",
        )
    }

    pub(crate) fn select_ledger_backup(
        &self,
        prompt: &str,
    ) -> Result<Option<PathBuf>, CommandError> {
        self.select_with(
            LedgerBackupSelectionProvider::new(&self.state, &self.store),
            prompt,
            "No backups available.",
        )
    }

    pub(crate) fn select_config_backup(
        &self,
        prompt: &str,
    ) -> Result<Option<PathBuf>, CommandError> {
        self.select_with(
            ConfigBackupSelectionProvider::new(&self.store),
            prompt,
            "No configuration backups found.",
        )
    }

    pub(crate) fn select_account_index(&self, prompt: &str) -> Result<Option<usize>, CommandError> {
        self.select_with(
            AccountSelectionProvider::new(&self.state),
            prompt,
            "No accounts available.",
        )
    }

    pub(crate) fn select_category_index(
        &self,
        prompt: &str,
    ) -> Result<Option<usize>, CommandError> {
        self.select_with(
            CategorySelectionProvider::new(&self.state),
            prompt,
            "No categories available.",
        )
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
        match data.id {
            Some(id) => {
                let ledger = self.current_ledger_mut()?;
                let updated_name = {
                    let account = ledger.account_mut(id).ok_or_else(|| {
                        CommandError::InvalidArguments("Account not found".into())
                    })?;
                    account.name = data.name.clone();
                    account.kind = data.kind;
                    account.category_id = data.category_id;
                    account.opening_balance = data.opening_balance;
                    account.notes = data.notes;
                    account.name.clone()
                };
                ledger.touch();
                output_success(format!("Account `{}` updated.", updated_name));
            }
            None => {
                let mut account = Account::new(data.name.clone(), data.kind);
                account.category_id = data.category_id;
                account.opening_balance = data.opening_balance;
                account.notes = data.notes;
                self.current_ledger_mut()?.add_account(account);
                output_success(format!("Account `{}` added.", data.name));
            }
        }
        Ok(())
    }

    fn apply_category_form(&mut self, data: CategoryFormData) -> CommandResult {
        match data.id {
            Some(id) => {
                let ledger = self.current_ledger_mut()?;
                let updated_name = {
                    let category = ledger.category_mut(id).ok_or_else(|| {
                        CommandError::InvalidArguments("Category not found".into())
                    })?;
                    category.name = data.name.clone();
                    category.kind = data.kind;
                    category.parent_id = data.parent_id;
                    category.is_custom = data.is_custom;
                    category.notes = data.notes;
                    category.name.clone()
                };
                ledger.touch();
                output_success(format!("Category `{}` updated.", updated_name));
            }
            None => {
                let mut category = Category::new(data.name.clone(), data.kind);
                category.parent_id = data.parent_id;
                category.is_custom = data.is_custom;
                category.notes = data.notes;
                self.current_ledger_mut()?.add_category(category);
                output_success(format!("Category `{}` added.", data.name));
            }
        }
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

        let summary = {
            let ledger = self.current_ledger()?;
            self.transaction_summary_line(ledger, &transaction)
        };

        if let Some(name) = simulation {
            {
                let ledger = self.current_ledger_mut()?;
                ledger
                    .add_simulation_transaction(name, transaction)
                    .map_err(CommandError::from_ledger)?;
            }
            output_success(format!(
                "Transaction saved to simulation `{}`: {}",
                name, summary
            ));
        } else {
            let id = {
                let ledger = self.current_ledger_mut()?;
                ledger.add_transaction(transaction)
            };
            let summary = {
                let ledger = self.current_ledger()?;
                let txn = ledger
                    .transaction(id)
                    .expect("transaction just added should exist");
                self.transaction_summary_line(ledger, txn)
            };
            output_success(format!("Transaction saved: {}", summary));
        }
        Ok(())
    }

    fn apply_transaction_update(&mut self, data: TransactionFormData) -> CommandResult {
        let txn_id = data.id.ok_or_else(|| {
            CommandError::InvalidArguments("transaction identifier missing".into())
        })?;
        {
            let ledger = self.current_ledger_mut()?;
            let transaction = ledger
                .transaction_mut(txn_id)
                .ok_or_else(|| CommandError::InvalidArguments("Transaction not found".into()))?;
            Self::populate_transaction_from_form(transaction, &data);
            ledger.refresh_recurrence_metadata();
            ledger.touch();
        }
        let summary = {
            let ledger = self.current_ledger()?;
            let txn = ledger
                .transaction(txn_id)
                .expect("transaction should exist after update");
            self.transaction_summary_line(ledger, txn)
        };
        output_success(format!("Transaction updated: {}", summary));
        Ok(())
    }

    fn remove_transaction_by_index(&mut self, index: usize) -> CommandResult {
        let (transaction, summary) = {
            let ledger = self.current_ledger()?;
            let txn = ledger.transactions.get(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            let summary = self.transaction_summary_line(ledger, txn);
            (txn.clone(), summary)
        };
        let ledger = self.current_ledger_mut()?;
        if ledger.remove_transaction(transaction.id).is_none() {
            return Err(CommandError::InvalidArguments(
                "transaction index out of range".into(),
            ));
        }
        output_success(format!("Transaction removed: {}", summary));
        Ok(())
    }

    fn display_transaction(&self, index: usize) -> CommandResult {
        let ledger = self.current_ledger()?;
        let txn = ledger.transactions.get(index).ok_or_else(|| {
            CommandError::InvalidArguments("transaction index out of range".into())
        })?;

        output_info(format!("Transaction [{}]", index));
        let route = self.describe_transaction_route(ledger, txn);
        output_info(format!("Route: {}", route));
        let category = txn
            .category_id
            .and_then(|id| self.lookup_category_name(ledger, id))
            .unwrap_or_else(|| "Uncategorized".into());
        output_info(format!("Category: {}", category));
        output_info(format!(
            "Scheduled: {}",
            self.format_date(ledger, txn.scheduled_date)
        ));
        let budget = format_currency_value(
            txn.budgeted_amount,
            &ledger.transaction_currency(txn),
            &ledger.locale,
            &ledger.format,
        );
        output_info(format!("Budgeted: {}", budget));
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
            output_info(format!("Actual: {} on {}", amount_label, date_label));
        }
        output_info(format!("Status: {:?}", txn.status));
        if let Some(hint) = self.transaction_recurrence_hint(txn) {
            output_info(format!("Recurrence: {}", hint));
        } else if txn.recurrence.is_some() || txn.recurrence_series_id.is_some() {
            output_info("Recurrence: linked instance");
        }
        if let Some(notes) = &txn.notes {
            if !notes.trim().is_empty() {
                output_info(format!("Notes: {}", notes));
            }
        }
        Ok(())
    }

    pub(crate) fn run_account_add_wizard(&mut self) -> CommandResult {
        self.ensure_base_mode("Account creation")?;
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: add account <name> <kind>".into(),
            ));
        }

        let (existing_names, category_options) = {
            let ledger = self.current_ledger()?;
            let names: HashSet<String> = ledger.accounts.iter().map(|a| a.name.clone()).collect();
            let categories = self.account_category_options(ledger);
            (names, categories)
        };

        let wizard = AccountWizard::new_create(existing_names, category_options);
        let mut interaction = DialoguerInteraction::new(&self.theme);
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                output_info("Account creation cancelled.");
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

        let (existing_names, category_options, initial) = {
            let ledger = self.current_ledger()?;
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
            (names, categories, initial)
        };

        let wizard = AccountWizard::new_edit(existing_names, initial, category_options);
        let mut interaction = DialoguerInteraction::new(&self.theme);
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                output_info("Account update cancelled.");
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

        let (existing_names, parent_options) = {
            let ledger = self.current_ledger()?;
            let names: HashSet<String> = ledger.categories.iter().map(|c| c.name.clone()).collect();
            let parents = self.category_parent_options(ledger, &HashSet::new());
            (names, parents)
        };

        let wizard = CategoryWizard::new_create(existing_names, parent_options);
        let mut interaction = DialoguerInteraction::new(&self.theme);
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                output_info("Category creation cancelled.");
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

        let (existing_names, parent_options, initial, allow_kind_change, allow_custom_change) = {
            let ledger = self.current_ledger()?;
            if index >= ledger.categories.len() {
                return Err(CommandError::InvalidArguments(
                    "category index out of range".into(),
                ));
            }
            let category = &ledger.categories[index];
            let names: HashSet<String> = ledger.categories.iter().map(|c| c.name.clone()).collect();
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
            (
                names,
                parents,
                initial,
                allow_kind_change,
                allow_custom_change,
            )
        };

        if !allow_kind_change || !allow_custom_change {
            output_info("Note: predefined categories cannot change their type or custom flag.");
        }

        let wizard = CategoryWizard::new_edit(
            existing_names,
            initial,
            parent_options,
            allow_kind_change,
            allow_custom_change,
        );
        let mut interaction = DialoguerInteraction::new(&self.theme);
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                output_info("Category update cancelled.");
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
        self.state.set_ledger(ledger, path, name);
        self.state.set_active_simulation(None);
    }

    pub(crate) fn command(&self, name: &str) -> Option<&CommandDefinition> {
        self.registry.get(name)
    }

    pub(crate) fn command_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.registry.names().collect();
        names.sort_unstable();
        names
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
        output_success("New ledger created.");
        Ok(())
    }

    fn prompt_budget_period(&self) -> Result<BudgetPeriod, CommandError> {
        let interval = self.prompt_time_interval(None)?;
        Ok(BudgetPeriod(interval))
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
                "usage: new-ledger <name> <period>".into(),
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
        output_success("New ledger created.");
        Ok(())
    }

    pub(crate) fn load_ledger(&mut self, path: &Path) -> CommandResult {
        let report = self
            .store
            .load_from_path(path)
            .map_err(CommandError::from_ledger)?;
        self.set_ledger(report.ledger, Some(path.to_path_buf()), None);
        output_success(format!("Ledger loaded from {}.", path.display()));
        self.report_load(&report.warnings, &report.migrations);
        let _ = self.store.record_last_ledger(None);
        Ok(())
    }

    pub(crate) fn save_to_path(&mut self, path: &Path) -> CommandResult {
        let mut snapshot = self.current_ledger()?.clone();
        self.store
            .save_to_path(&mut snapshot, path)
            .map_err(CommandError::from_ledger)?;
        self.state.set_path(Some(path.to_path_buf()));
        self.state.set_named(None);
        output_success(format!("Ledger saved to {}.", path.display()));
        Ok(())
    }

    pub(crate) fn load_named_ledger(&mut self, name: &str) -> CommandResult {
        let report = self
            .store
            .load_named(name)
            .map_err(CommandError::from_ledger)?;
        let path = self.store.ledger_path(name);
        self.set_ledger(report.ledger, Some(path.clone()), Some(name.to_string()));
        output_success(format!("Ledger `{}` loaded from {}.", name, path.display()));
        self.report_load(&report.warnings, &report.migrations);
        let _ = self.store.record_last_ledger(Some(name));
        Ok(())
    }

    pub(crate) fn save_named_ledger(&mut self, name: &str) -> CommandResult {
        let mut snapshot = self.current_ledger()?.clone();
        let path = self
            .store
            .save_named(&mut snapshot, name)
            .map_err(CommandError::from_ledger)?;
        self.state.set_path(Some(path.clone()));
        self.state.set_named(Some(name.to_string()));
        output_success(format!("Ledger `{}` saved to {}.", name, path.display()));
        let _ = self.store.record_last_ledger(Some(name));
        Ok(())
    }

    pub(crate) fn create_backup(&mut self, name: &str) -> CommandResult {
        let path = self
            .store
            .backup_named(name)
            .map_err(CommandError::from_ledger)?;
        output_success(format!("Backup created at {}.", path.display()));
        Ok(())
    }

    pub(crate) fn list_backups(&self, name: &str) -> CommandResult {
        let backups = self
            .store
            .list_backups(name)
            .map_err(CommandError::from_ledger)?;
        if backups.is_empty() {
            output_warning("No backups available.");
            return Ok(());
        }
        output_info("Available backups:");
        for (idx, backup) in backups.iter().enumerate() {
            let created = backup.timestamp.with_timezone(&Local);
            let file_name = backup
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| backup.path.display().to_string());
            output_info(format!(
                "  {:>2}. {} (Created: {})",
                idx + 1,
                file_name,
                created.format("%Y-%m-%d %H:%M")
            ));
            output_info(format!("      {}", backup.path.display()));
        }
        Ok(())
    }

    pub(crate) fn restore_backup(&mut self, name: &str, reference: &str) -> CommandResult {
        let backups = self
            .store
            .list_backups(name)
            .map_err(CommandError::from_ledger)?;
        if backups.is_empty() {
            return Err(CommandError::InvalidArguments(
                "no backups available to restore".into(),
            ));
        }
        let target = if let Ok(index) = reference.parse::<usize>() {
            backups
                .get(index)
                .ok_or_else(|| {
                    CommandError::InvalidArguments(format!("backup index {} out of range", index))
                })?
                .path
                .clone()
        } else {
            let matching = backups
                .iter()
                .find(|info| {
                    info.path
                        .file_name()
                        .and_then(|os| os.to_str())
                        .map(|name| name.contains(reference))
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    CommandError::InvalidArguments(format!(
                        "no backup matches reference `{}`",
                        reference
                    ))
                })?;
            matching.path.clone()
        };
        self.restore_backup_from_path(name, target)
    }

    pub(crate) fn restore_backup_from_path(
        &mut self,
        name: &str,
        target: PathBuf,
    ) -> CommandResult {
        let confirm = if self.mode == CliMode::Interactive {
            Confirm::with_theme(&self.theme)
                .with_prompt(format!(
                    "Restore ledger `{}` from {}?",
                    name,
                    target.display()
                ))
                .default(false)
                .interact()
                .map_err(CommandError::from)?
        } else {
            true
        };
        if !confirm {
            output_info("Operation cancelled.");
            return Ok(());
        }
        self.store
            .restore_backup(name, &target)
            .map_err(CommandError::from_ledger)?;
        self.load_named_ledger(name)
    }

    pub(crate) fn create_config_backup(&mut self, note: Option<String>) -> CommandResult {
        let ledger = self.current_ledger()?;
        let note = note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        let config = ConfigData::from_ledger(ledger);
        let snapshot = ConfigSnapshot::new(config, note);
        let path = self
            .store
            .create_config_backup(&snapshot)
            .map_err(CommandError::from_ledger)?;
        self.store
            .save_active_config(&snapshot.config)
            .map_err(CommandError::from_ledger)?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("configuration backup");
        let created = snapshot.created_at.with_timezone(&Local);
        output_success(format!(
            "Configuration backup saved: {} (Created: {})",
            file_name,
            created.format("%Y-%m-%d %H:%M")
        ));
        output_info("Stored in the `config_backups` directory.");
        Ok(())
    }

    pub(crate) fn list_config_backups(&self) -> CommandResult {
        let backups = self
            .store
            .list_config_backups()
            .map_err(CommandError::from_ledger)?;
        if backups.is_empty() {
            output_warning("No configuration backups found.");
            return Ok(());
        }
        output_info("Available configuration backups:");
        for (idx, backup) in backups.iter().enumerate() {
            let file_name = backup
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("config-backup");
            let created = backup.created_at.with_timezone(&Local);
            let mut line = format!(
                "  {:>2}. {} (Created: {})",
                idx + 1,
                file_name,
                created.format("%Y-%m-%d %H:%M")
            );
            if let Some(note) = backup
                .note
                .as_ref()
                .map(|n| n.trim())
                .filter(|n| !n.is_empty())
            {
                line.push_str(&format!(" (note: {})", note));
            }
            output_info(line);
        }
        Ok(())
    }

    pub(crate) fn restore_config_by_reference(&mut self, reference: &str) -> CommandResult {
        let backups = self
            .store
            .list_config_backups()
            .map_err(CommandError::from_ledger)?;
        if backups.is_empty() {
            return Err(CommandError::InvalidArguments(
                "no configuration backups available".into(),
            ));
        }
        let target_path = if let Ok(index_raw) = reference.parse::<usize>() {
            let zero_index = if index_raw > 0 && index_raw <= backups.len() {
                index_raw - 1
            } else {
                index_raw
            };
            backups
                .get(zero_index)
                .ok_or_else(|| {
                    CommandError::InvalidArguments(format!(
                        "configuration backup index {} out of range",
                        reference
                    ))
                })?
                .path
                .clone()
        } else {
            backups
                .iter()
                .find(|info| {
                    info.path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.contains(reference))
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    CommandError::InvalidArguments(format!(
                        "no configuration backup matches reference `{}`",
                        reference
                    ))
                })?
                .path
                .clone()
        };
        self.restore_config_from_path(target_path)
    }

    pub(crate) fn restore_config_from_path(&mut self, path: PathBuf) -> CommandResult {
        let snapshot = self
            .store
            .load_config_snapshot(&path)
            .map_err(CommandError::from_ledger)?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("configuration backup");
        let created = snapshot.created_at.with_timezone(&Local);
        output_info(format!("Selected backup: {}", file_name));
        output_info(format!("Created: {}", created.format("%Y-%m-%d %H:%M")));
        if let Some(note) = snapshot
            .note
            .as_ref()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
        {
            output_info(format!("Note: {}", note));
        }
        output_info(format!("Base currency: {}", snapshot.config.base_currency));
        output_info(format!("Locale: {}", snapshot.config.locale.language_tag));
        output_info(format!(
            "Currency display: {:?} | Negative style: {:?}",
            snapshot.config.currency_display, snapshot.config.negative_style
        ));
        output_info(format!(
            "Screen reader: {} | High contrast: {}",
            if snapshot.config.screen_reader_mode {
                "on"
            } else {
                "off"
            },
            if snapshot.config.high_contrast_mode {
                "on"
            } else {
                "off"
            }
        ));
        output_info(format!(
            "Valuation policy: {:?}",
            snapshot.config.valuation_policy
        ));

        let confirm = if self.mode == CliMode::Interactive {
            Confirm::with_theme(&self.theme)
                .with_prompt("Restore configuration from this backup?")
                .default(false)
                .interact()
                .map_err(CommandError::from)?
        } else {
            true
        };
        if !confirm {
            output_warning("Operation cancelled.");
            return Ok(());
        }

        {
            let ledger = self.current_ledger_mut()?;
            snapshot.config.apply_to_ledger(ledger);
            ledger.touch();
        }
        self.store
            .save_active_config(&snapshot.config)
            .map_err(CommandError::from_ledger)?;
        output_success(format!(
            "Configuration restored from {} (Created: {})",
            file_name,
            created.format("%Y-%m-%d %H:%M")
        ));
        Ok(())
    }

    pub(crate) fn add_account_interactive(&mut self) -> CommandResult {
        self.run_account_add_wizard()
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
        let ledger = self.current_ledger_mut()?;
        ledger.add_account(account);
        output_success("Account added.");
        Ok(())
    }

    pub(crate) fn add_category_interactive(&mut self) -> CommandResult {
        self.run_category_add_wizard()
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
        let ledger = self.current_ledger_mut()?;
        ledger.add_category(category);
        output_success("Category added.");
        Ok(())
    }

    pub(crate) fn add_transaction_interactive(&mut self) -> CommandResult {
        if self.mode != CliMode::Interactive {
            return Err(CommandError::InvalidArguments(
                "usage: add transaction <from_account_index> <to_account_index> <YYYY-MM-DD> <amount>"
                    .into(),
            ));
        }
        let sim = self.active_simulation_name().map(|s| s.to_string());
        self.run_transaction_add_wizard(sim.as_deref())
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

        let (from_id, to_id) = {
            let ledger = self.current_ledger()?;
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
            (ledger.accounts[from_index].id, ledger.accounts[to_index].id)
        };

        let transaction = Transaction::new(from_id, to_id, None, date, amount);
        let summary = {
            let ledger = self.current_ledger()?;
            self.transaction_summary_line(ledger, &transaction)
        };

        if let Some(sim_name) = sim {
            {
                let ledger = self.current_ledger_mut()?;
                ledger
                    .add_simulation_transaction(&sim_name, transaction)
                    .map_err(CommandError::from_ledger)?;
            }
            output_success(format!(
                "Transaction saved to simulation `{}`: {}",
                sim_name, summary
            ));
        } else {
            let id = {
                let ledger = self.current_ledger_mut()?;
                ledger.add_transaction(transaction)
            };
            let summary = {
                let ledger = self.current_ledger()?;
                let txn = ledger
                    .transaction(id)
                    .expect("transaction just added should exist");
                self.transaction_summary_line(ledger, txn)
            };
            output_success(format!("Transaction saved: {}", summary));
        }
        Ok(())
    }

    fn run_transaction_add_wizard(&mut self, simulation: Option<&str>) -> CommandResult {
        let ledger = self.current_ledger()?;
        if ledger.accounts.is_empty() {
            return Err(CommandError::Message(
                "Add at least one account before creating transactions".into(),
            ));
        }
        let accounts = self.transaction_account_options(ledger);
        let categories = self.account_category_options(ledger);
        let today = Utc::now().date_naive();
        let min_date = ledger.created_at.date_naive();
        let default_status = if simulation.is_some() {
            TransactionStatus::Simulated
        } else {
            TransactionStatus::Planned
        };
        let wizard =
            TransactionWizard::new_create(accounts, categories, today, min_date, default_status);
        let mut interaction = DialoguerInteraction::new(&self.theme);
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                output_info("Transaction creation cancelled.");
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
        let (accounts, categories, initial, created_at) = {
            let ledger = self.current_ledger()?;
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
            (accounts, categories, initial, created_at)
        };
        let today = Utc::now().date_naive();
        let min_date = created_at.date_naive();
        let wizard = TransactionWizard::new_edit(accounts, categories, today, min_date, initial);
        let mut interaction = DialoguerInteraction::new(&self.theme);
        match FormEngine::new(&wizard).run(&mut interaction).unwrap() {
            FormResult::Cancelled => {
                output_info("Transaction update cancelled.");
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
        if self.current_ledger()?.transactions.is_empty() {
            output_warning("No transactions available.");
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
        if self.current_ledger()?.transactions.is_empty() {
            output_warning("No transactions available.");
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
        if self.current_ledger()?.transactions.is_empty() {
            output_warning("No transactions available.");
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
        if self.current_ledger()?.transactions.is_empty() {
            output_warning("No transactions available.");
            return Ok(());
        }
        let selection = self.transaction_index_from_arg(args.first().copied(), usage, prompt)?;
        let Some(idx) = selection else {
            return Ok(());
        };

        let (scheduled_default, budget_default) = {
            let ledger = self.current_ledger()?;
            let txn = ledger.transactions.get(idx).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            (
                txn.scheduled_date,
                txn.actual_amount.unwrap_or(txn.budgeted_amount),
            )
        };

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

        let ledger = self.current_ledger_mut()?;
        let txn = ledger.transactions.get_mut(idx).ok_or_else(|| {
            CommandError::InvalidArguments("transaction index out of range".into())
        })?;
        txn.mark_completed(actual_date, amount);
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        output_success(format!("Transaction {} marked completed", idx));
        Ok(())
    }

    pub(crate) fn transaction_complete(&mut self, args: &[&str]) -> CommandResult {
        self.transaction_complete_internal(
            args,
            "usage: transaction complete <transaction_index> <YYYY-MM-DD> <amount>",
            "Select a transaction to complete:",
        )
    }

    pub(crate) fn legacy_complete(&mut self, args: &[&str]) -> CommandResult {
        self.transaction_complete_internal(
            args,
            "usage: complete <transaction_index> <YYYY-MM-DD> <amount>",
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
        let ledger = self.current_ledger()?;
        if ledger.accounts.is_empty() {
            output_warning("No accounts defined.");
            return Ok(());
        }

        output_section("Accounts");
        for (idx, account) in ledger.accounts.iter().enumerate() {
            output_info(format!(
                "  [{idx:>3}] {name} ({kind:?})",
                idx = idx,
                name = account.name,
                kind = account.kind
            ));
        }
        Ok(())
    }

    pub(crate) fn list_categories(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        if ledger.categories.is_empty() {
            output_warning("No categories defined.");
            return Ok(());
        }

        output_section("Categories");
        for (idx, category) in ledger.categories.iter().enumerate() {
            let parent_marker = if category.parent_id.is_some() {
                " [child]"
            } else {
                ""
            };
            output_info(format!(
                "  [{idx:>3}] {name} ({kind:?}){parent_marker}",
                idx = idx,
                name = category.name,
                kind = category.kind,
                parent_marker = parent_marker
            ));
        }
        Ok(())
    }

    pub(crate) fn list_transactions(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        if ledger.transactions.is_empty() {
            output_warning("No transactions recorded.");
            return Ok(());
        }

        output_section("Transactions");
        for (idx, txn) in ledger.transactions.iter().enumerate() {
            let route = self.describe_transaction_route(ledger, txn);
            let category = txn
                .category_id
                .and_then(|id| self.lookup_category_name(ledger, id))
                .unwrap_or_else(|| "Uncategorized".into());
            let status = format!("{:?}", txn.status);
            let txn_currency = ledger.transaction_currency(txn);
            let scheduled = self.format_date(ledger, txn.scheduled_date);
            let budget_amount = format_currency_value(
                txn.budgeted_amount,
                &txn_currency,
                &ledger.locale,
                &ledger.format,
            );
            output_info(format!(
                "  [{idx:>3}] {date} | {amount} | {status:<10} | {route} ({category})",
                idx = idx,
                date = scheduled,
                amount = budget_amount,
                status = status,
                route = route,
                category = category
            ));
            if let Some(actual_date) = txn.actual_date {
                if let Some(actual_amount) = txn.actual_amount {
                    let formatted_date = self.format_date(ledger, actual_date);
                    let formatted_amount = format_currency_value(
                        actual_amount,
                        &txn_currency,
                        &ledger.locale,
                        &ledger.format,
                    );
                    output_info(format!(
                        "        actual {} | {}",
                        formatted_date, formatted_amount
                    ));
                }
            }
            if let Some(hint) = self.transaction_recurrence_hint(txn) {
                output_info(format!("        {}", hint));
            } else if txn.recurrence_series_id.is_some() {
                output_info("        [instance] scheduled entry from recurrence");
            }
        }
        Ok(())
    }

    pub(crate) fn show_budget_summary(&self, args: &[&str]) -> CommandResult {
        let ledger = self.current_ledger()?;
        let today = Utc::now().date_naive();

        let (simulation_name, remainder) =
            if !args.is_empty() && ledger.simulation(args[0]).is_some() {
                (Some(args[0]), &args[1..])
            } else {
                (None, args)
            };

        let (window, scope) = self.resolve_summary_window(ledger, remainder, today)?;

        if let Some(name) = simulation_name {
            let impact = ledger
                .summarize_simulation_in_window(name, window, scope)
                .map_err(CommandError::from_ledger)?;
            self.print_simulation_impact(ledger, &impact);
            return Ok(());
        }

        let summary = ledger.summarize_window_scope(window, scope);
        self.print_budget_summary(ledger, &summary);
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
                let window = DateWindow::new(start, end).map_err(CommandError::from_ledger)?;
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
            return DateWindow::new(today, end).map_err(CommandError::from_ledger);
        }
        if matches!(args[0].to_lowercase().as_str(), "custom" | "range") {
            if args.len() < 3 {
                return Err(CommandError::InvalidArguments(
                    "usage: forecast custom <start YYYY-MM-DD> <end YYYY-MM-DD>".into(),
                ));
            }
            let start = parse_date(args[1])?;
            let end = parse_date(args[2])?;
            return DateWindow::new(start, end).map_err(CommandError::from_ledger);
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
        DateWindow::new(today, end).map_err(CommandError::from_ledger)
    }

    fn print_budget_summary(&self, ledger: &Ledger, summary: &BudgetSummary) {
        let end_display = summary
            .window
            .end
            .checked_sub_signed(Duration::days(1))
            .unwrap_or(summary.window.end);
        output_section(format!(
            "{:?} {} → {}",
            summary.scope,
            self.format_date(ledger, summary.window.start),
            self.format_date(ledger, end_display)
        ));

        output_info(format!(
            "Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, summary.totals.budgeted),
            self.format_amount(ledger, summary.totals.real),
            self.format_amount(ledger, summary.totals.remaining),
            self.format_amount(ledger, summary.totals.variance)
        ));

        if let Some(percent) = summary.totals.percent_used {
            output_info(format!("Usage: {:.1}%", percent));
        }

        output_info(format!("Status: {:?}", summary.totals.status));

        if summary.incomplete_transactions > 0 {
            output_warning(format!(
                "{} incomplete transactions",
                summary.incomplete_transactions
            ));
        }

        if summary.orphaned_transactions > 0 {
            output_warning(format!(
                "{} transactions reference unknown accounts or categories",
                summary.orphaned_transactions
            ));
        }

        if summary.per_category.is_empty() {
            output_info("No category data for this window.");
        } else {
            output_info("Categories:");
            for cat in summary.per_category.iter().take(5) {
                output_info(format!(
                    "  {:<20} {} budgeted / {} real ({:?})",
                    cat.name,
                    self.format_amount(ledger, cat.totals.budgeted),
                    self.format_amount(ledger, cat.totals.real),
                    cat.totals.status
                ));
            }
            if summary.per_category.len() > 5 {
                output_info(format!(
                    "  ... {} more categories",
                    summary.per_category.len() - 5
                ));
            }
        }

        if !summary.per_account.is_empty() {
            output_info("Accounts:");
            for acct in summary.per_account.iter().take(5) {
                output_info(format!(
                    "  {:<20} {} budgeted / {} real ({:?})",
                    acct.name,
                    self.format_amount(ledger, acct.totals.budgeted),
                    self.format_amount(ledger, acct.totals.real),
                    acct.totals.status
                ));
            }
            if summary.per_account.len() > 5 {
                output_info(format!(
                    "  ... {} more accounts",
                    summary.per_account.len() - 5
                ));
            }
        }

        if !summary.disclosures.is_empty() {
            output_info("Disclosures:");
            for note in &summary.disclosures {
                output_info(format!("  - {}", note));
            }
        }
    }

    fn print_simulation_impact(&self, ledger: &Ledger, impact: &SimulationBudgetImpact) {
        output_section(format!("Simulation `{}`", impact.simulation_name));
        output_info("Base totals:");
        output_info(format!(
            "  Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, impact.base.totals.budgeted),
            self.format_amount(ledger, impact.base.totals.real),
            self.format_amount(ledger, impact.base.totals.remaining),
            self.format_amount(ledger, impact.base.totals.variance)
        ));
        output_info("Simulated totals:");
        output_info(format!(
            "  Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, impact.simulated.totals.budgeted),
            self.format_amount(ledger, impact.simulated.totals.real),
            self.format_amount(ledger, impact.simulated.totals.remaining),
            self.format_amount(ledger, impact.simulated.totals.variance)
        ));
        output_info("Delta:");
        output_info(format!(
            "  Budgeted: {} | Real: {} | Remaining: {} | Variance: {}",
            self.format_amount(ledger, impact.delta.budgeted),
            self.format_amount(ledger, impact.delta.real),
            self.format_amount(ledger, impact.delta.remaining),
            self.format_amount(ledger, impact.delta.variance)
        ));
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
        output_section(format!(
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
        output_info(format!(
            "Occurrences: {instance_count} total | {existing_count} already scheduled | {generated_count} projected"
        ));
        output_info(format!(
            "Status mix: {overdue} overdue | {pending} pending | {future} future"
        ));
        output_info(format!(
            "Projected totals: Inflow {} | Outflow {} | Net {}",
            self.format_amount(ledger, totals.projected_inflow),
            self.format_amount(ledger, totals.projected_outflow),
            self.format_amount(ledger, totals.net)
        ));
        output_info(format!(
            "Budget impact: Budgeted {} | Real {} | Remaining {} | Variance {}",
            self.format_amount(ledger, report.summary.totals.budgeted),
            self.format_amount(ledger, report.summary.totals.real),
            self.format_amount(ledger, report.summary.totals.remaining),
            self.format_amount(ledger, report.summary.totals.variance)
        ));
        if !report.summary.disclosures.is_empty() {
            output_info("Disclosures:");
            for note in &report.summary.disclosures {
                output_info(format!("  - {}", note));
            }
        }

        if report.forecast.transactions.is_empty() {
            output_info("No additional projections required within this window.");
            return;
        }

        output_info("Upcoming projections:");
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
            output_info(format!(
                "  {date} | {amount} | {status:<8} | {route} ({category})",
                date = self.format_date(ledger, item.transaction.scheduled_date),
                amount = amount,
                status = status,
                route = route,
                category = category
            ));
        }
        if report.forecast.transactions.len() > 8 {
            output_info(format!(
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
        let ledger = self.current_ledger()?;
        let today = Utc::now().date_naive();
        let snapshot_map: HashMap<Uuid, RecurrenceSnapshot> = ledger
            .recurrence_snapshots(today)
            .into_iter()
            .map(|snap| (snap.series_id, snap))
            .collect();
        if snapshot_map.is_empty() {
            output_warning("No recurring schedules defined.");
            return Ok(());
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

        output_section("Recurring schedules");
        let mut shown = 0;
        for (index, txn, snapshot) in entries {
            if !filter.matches(snapshot) {
                continue;
            }
            shown += 1;
            self.print_recurrence_entry(ledger, index, txn, snapshot);
        }
        if shown == 0 {
            output_info("No recurring entries match the requested filter.");
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
        output_info(format!(
            "[{idx:>3}] {route} | {cat} | every {freq} | next {next} | overdue {overdue} | pending {pending}",
            idx = index,
            route = route,
            cat = category,
            freq = snapshot.interval_label,
            next = next_due,
            overdue = snapshot.overdue,
            pending = snapshot.pending
        ));
        output_info(format!(
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
        let (scheduled_date, existing) = {
            let ledger = self.current_ledger()?;
            let txn = ledger.transactions.get(index).ok_or_else(|| {
                CommandError::InvalidArguments("transaction index out of range".into())
            })?;
            (txn.scheduled_date, txn.recurrence.clone())
        };
        let recurrence = self.prompt_recurrence(scheduled_date, existing.as_ref())?;
        let ledger = self.current_ledger_mut()?;
        let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
            CommandError::InvalidArguments("transaction index out of range".into())
        })?;
        txn.set_recurrence(Some(recurrence));
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        output_success(format!("Recurrence updated for transaction {}.", index));
        Ok(())
    }

    pub(crate) fn recurrence_clear(&mut self, index: usize) -> CommandResult {
        self.ensure_base_mode("Recurrence removal")?;
        let ledger = self.current_ledger_mut()?;
        let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
            CommandError::InvalidArguments("transaction index out of range".into())
        })?;
        if txn.recurrence.is_none() {
            output_warning("Transaction has no recurrence defined.");
            return Ok(());
        }
        txn.set_recurrence(None);
        txn.recurrence_series_id = None;
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        output_success(format!("Recurrence removed from transaction {}.", index));
        Ok(())
    }

    pub(crate) fn recurrence_set_status(
        &mut self,
        index: usize,
        status: RecurrenceStatus,
    ) -> CommandResult {
        self.ensure_base_mode("Recurrence status change")?;
        let ledger = self.current_ledger_mut()?;
        let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
            CommandError::InvalidArguments("transaction index out of range".into())
        })?;
        let recurrence = txn.recurrence.as_mut().ok_or_else(|| {
            CommandError::InvalidArguments("transaction has no recurrence".into())
        })?;
        recurrence.status = status.clone();
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        output_success(format!(
            "Recurrence status set to {:?} for transaction {}.",
            status, index
        ));
        Ok(())
    }

    pub(crate) fn recurrence_skip_date(&mut self, index: usize, date: NaiveDate) -> CommandResult {
        self.ensure_base_mode("Recurrence exception editing")?;
        let ledger = self.current_ledger_mut()?;
        let txn = ledger.transactions.get_mut(index).ok_or_else(|| {
            CommandError::InvalidArguments("transaction index out of range".into())
        })?;
        let recurrence = txn.recurrence.as_mut().ok_or_else(|| {
            CommandError::InvalidArguments("transaction has no recurrence".into())
        })?;
        if recurrence.exceptions.contains(&date) {
            output_info(format!(
                "Date {} already marked as skipped for this recurrence.",
                date
            ));
            return Ok(());
        }
        recurrence.exceptions.push(date);
        recurrence.exceptions.sort();
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        output_success(format!(
            "Added skip date {} for transaction {}.",
            date, index
        ));
        Ok(())
    }

    pub(crate) fn recurrence_sync(&mut self, reference: NaiveDate) -> CommandResult {
        self.ensure_base_mode("Recurrence synchronization")?;
        let ledger = self.current_ledger_mut()?;
        let created = ledger.materialize_due_recurrences(reference);
        if created == 0 {
            output_info("All due recurring instances already exist.");
        } else {
            output_success(format!(
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
                let ledger = self.current_ledger()?;
                if ledger.simulation(&name).is_none() {
                    Err(CommandError::InvalidArguments(format!(
                        "simulation `{}` not found",
                        name
                    )))
                } else {
                    Ok(Some(name))
                }
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
        let ledger = self.current_ledger()?;
        let sim = ledger.simulation(sim_name).ok_or_else(|| {
            CommandError::InvalidArguments(format!("simulation `{}` not found", sim_name))
        })?;
        output_info(format!("Simulation `{}` ({:?})", sim.name, sim.status));
        if sim.changes.is_empty() {
            output_info("No pending changes.");
        } else {
            for (idx, change) in sim.changes.iter().enumerate() {
                match change {
                    SimulationChange::AddTransaction { transaction } => output_info(format!(
                        "  [{:>2}] Add transaction {} -> {} on {} (budgeted {:.2})",
                        idx,
                        transaction.from_account,
                        transaction.to_account,
                        transaction.scheduled_date,
                        transaction.budgeted_amount
                    )),
                    SimulationChange::ModifyTransaction(patch) => output_info(format!(
                        "  [{:>2}] Modify transaction {}",
                        idx, patch.transaction_id
                    )),
                    SimulationChange::ExcludeTransaction { transaction_id } => output_info(
                        format!("  [{:>2}] Exclude transaction {}", idx, transaction_id),
                    ),
                }
            }
        }
        Ok(())
    }

    pub(crate) fn simulation_add_transaction(&mut self, sim_name: &str) -> CommandResult {
        self.run_transaction_add_wizard(Some(sim_name))
    }

    pub(crate) fn simulation_exclude_transaction(&mut self, sim_name: &str) -> CommandResult {
        let txn_id = self.select_transaction_id("Exclude which transaction?")?;
        self.current_ledger_mut()?
            .exclude_transaction_in_simulation(sim_name, txn_id)
            .map_err(CommandError::from_ledger)?;
        output_success(format!("Transaction {} excluded in `{}`", txn_id, sim_name));
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

        self.current_ledger_mut()?
            .modify_transaction_in_simulation(sim_name, patch)
            .map_err(CommandError::from_ledger)?;
        output_success(format!("Transaction {} modified in `{}`", txn_id, sim_name));
        Ok(())
    }

    fn select_transaction_id(&self, prompt: &str) -> Result<Uuid, CommandError> {
        let ledger = self.current_ledger()?;
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
                    idx, txn.from_account, txn.to_account, txn.scheduled_date, txn.budgeted_amount
                )
            })
            .collect();
        let selection = Select::with_theme(&self.theme)
            .with_prompt(prompt)
            .items(&items)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        Ok(ledger.transactions[selection].id)
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
pub enum CliError {
    #[error("{0}")]
    Internal(String),
    #[error(transparent)]
    Readline(#[from] ReadlineError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Ledger not loaded. Use `new-ledger` or `load` first.")]
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
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Dialoguer(#[from] dialoguer::Error),
    #[error("exit requested")]
    ExitRequested,
}

impl CommandError {
    pub(crate) fn from_ledger(error: LedgerError) -> Self {
        CommandError::Ledger(error)
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

#[cfg(test)]
pub(crate) fn process_script(lines: &[&str]) -> Result<CliState, CliError> {
    let mut app = ShellContext::new(CliMode::Script)?;
    for line in lines {
        match app.process_line(line)? {
            LoopControl::Continue => {}
            LoopControl::Exit => break,
        }
    }
    Ok(app.state)
}

impl CliState {
    #[cfg(test)]
    pub fn ledger(&self) -> Option<&Ledger> {
        self.ledger.as_ref()
    }
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
    use crate::currency::{CurrencyDisplay, LocaleConfig, NegativeStyle, ValuationPolicy};
    use crate::ledger::{Simulation, SimulationStatus};
    use crate::utils::persistence::{ConfigData, ConfigSnapshot, LedgerStore};
    use chrono::{NaiveDate, Utc};
    use std::fs;
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn parse_line_handles_quotes() {
        let tokens =
            crate::cli::shell::parse_command_line("new-ledger \"Demo Ledger\" monthly").unwrap();
        assert_eq!(tokens, vec!["new-ledger", "Demo Ledger", "monthly"]);
    }

    #[test]
    fn script_runner_creates_ledger() {
        let state = process_script(&["new-ledger Demo 3 months", "exit"]).unwrap();
        let ledger = state.ledger().expect("ledger present");
        assert_eq!(ledger.name, "Demo");
        assert_eq!(ledger.budget_period.0.every, 3);
        assert_eq!(ledger.budget_period.0.unit, TimeUnit::Month);
    }

    #[test]
    fn script_can_save_and_load() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let setup_cmds: Vec<String> = vec![
            "new-ledger Testing every 2 weeks".into(),
            format!("save {}", path.display()),
            "exit".into(),
        ];
        let setup_refs: Vec<&str> = setup_cmds.iter().map(String::as_str).collect();
        process_script(&setup_refs).unwrap();

        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"Testing\""));

        let load_cmds: Vec<String> = vec![
            format!("load {}", path.display()),
            "summary".into(),
            "exit".into(),
        ];
        let load_refs: Vec<&str> = load_cmds.iter().map(String::as_str).collect();
        let state = process_script(&load_refs).unwrap();
        let ledger = state.ledger().expect("ledger present");
        assert_eq!(ledger.name, "Testing");
        assert_eq!(ledger.budget_period.0.every, 2);
        assert_eq!(ledger.budget_period.0.unit, TimeUnit::Week);
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

    fn state_with_ledger() -> CliState {
        let mut state = CliState::new();
        let ledger = sample_ledger();
        state.set_ledger(ledger, None, Some("sample".into()));
        state
    }

    #[test]
    fn account_selection_positive() {
        let state = state_with_ledger();
        let outcome = SelectionManager::new(AccountSelectionProvider::new(&state))
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
        let state = state_with_ledger();
        let outcome = SelectionManager::new(AccountSelectionProvider::new(&state))
            .choose_with("Select account", "No accounts available.", |_, _| Ok(None))
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn category_selection_paths() {
        let state = state_with_ledger();
        let outcome = SelectionManager::new(CategorySelectionProvider::new(&state))
            .choose_with("Select category", "No categories available.", |_, _| {
                Ok(Some(0))
            })
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Selected(0)));

        let outcome = SelectionManager::new(CategorySelectionProvider::new(&state))
            .choose_with("Select category", "No categories available.", |_, _| {
                Ok(None)
            })
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn transaction_selection_paths() {
        let state = state_with_ledger();
        let outcome = SelectionManager::new(TransactionSelectionProvider::new(&state))
            .choose_with(
                "Select transaction",
                "No transactions available.",
                |_, _| Ok(Some(0)),
            )
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Selected(0)));

        let outcome = SelectionManager::new(TransactionSelectionProvider::new(&state))
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
        app.state.set_ledger(ledger, None, Some("sample".into()));

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
        let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(5)).unwrap();
        let mut state = state_with_ledger();
        state.set_named(Some("Sample".into()));

        let backup_dir = store.base_dir().join("backups").join("sample");
        fs::create_dir_all(&backup_dir).unwrap();
        let backup_path = backup_dir.join("2024-01-01T00-00-00.json.bak");
        fs::write(&backup_path, "{}").unwrap();

        let outcome = SelectionManager::new(LedgerBackupSelectionProvider::new(&state, &store))
            .choose_with("Select backup", "No backups available.", |_, _| Ok(Some(0)))
            .unwrap();
        match outcome {
            SelectionOutcome::Selected(path) => assert_eq!(path, backup_path),
            _ => panic!("expected backup selection"),
        }

        let outcome = SelectionManager::new(LedgerBackupSelectionProvider::new(&state, &store))
            .choose_with("Select backup", "No backups available.", |_, _| Ok(None))
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }

    #[test]
    fn config_backup_selection_paths() {
        let temp = tempdir().unwrap();
        let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(5)).unwrap();
        let snapshot = ConfigSnapshot::new(
            ConfigData {
                base_currency: "USD".into(),
                locale: LocaleConfig::default(),
                currency_display: CurrencyDisplay::Symbol,
                negative_style: NegativeStyle::Sign,
                screen_reader_mode: false,
                high_contrast_mode: false,
                valuation_policy: ValuationPolicy::TransactionDate,
            },
            Some("baseline".into()),
        );
        let config_path = store.create_config_backup(&snapshot).unwrap();

        let outcome = SelectionManager::new(ConfigBackupSelectionProvider::new(&store))
            .choose_with(
                "Select config",
                "No configuration backups found.",
                |_, _| Ok(Some(0)),
            )
            .unwrap();
        match outcome {
            SelectionOutcome::Selected(path) => assert_eq!(path, config_path),
            _ => panic!("expected config selection"),
        }

        let outcome = SelectionManager::new(ConfigBackupSelectionProvider::new(&store))
            .choose_with(
                "Select config",
                "No configuration backups found.",
                |_, _| Ok(None),
            )
            .unwrap();
        assert!(matches!(outcome, SelectionOutcome::Cancelled));
    }
}
