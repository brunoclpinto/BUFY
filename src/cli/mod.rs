use std::{
    collections::HashMap,
    fmt,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

use chrono::NaiveDate;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rustyline::{error::ReadlineError, DefaultEditor};
use shell_words::split;
use strsim::levenshtein;

use crate::{
    errors::LedgerError,
    ledger::{
        account::AccountKind, category::CategoryKind, Account, BudgetPeriod, Category, Ledger,
        Recurrence, RecurrenceMode, TimeInterval, TimeUnit, Transaction,
    },
    utils::persistence,
};

const PROMPT_ARROW: &str = "⮞";

pub fn run_cli() -> Result<(), CliError> {
    let mode = if std::env::var_os("BUDGET_CORE_CLI_SCRIPT").is_some() {
        CliMode::Script
    } else {
        CliMode::Interactive
    };

    let mut app = CliApp::new(mode)?;

    match app.mode {
        CliMode::Interactive => app.run_loop(),
        CliMode::Script => {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line?;
                match app.process_line(&line) {
                    Ok(LoopControl::Continue) => {}
                    Ok(LoopControl::Exit) => break,
                    Err(err) => app.report_error(err)?,
                }
            }
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    Interactive,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoopControl {
    Continue,
    Exit,
}

type CommandAction = fn(&mut CliApp, &[&str]) -> CommandResult;
type CommandResult = Result<(), CommandError>;

#[derive(Clone)]
struct Command {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    action: CommandAction,
}

impl Command {
    fn new(
        name: &'static str,
        description: &'static str,
        usage: &'static str,
        action: CommandAction,
    ) -> Self {
        Self {
            name,
            description,
            usage,
            action,
        }
    }
}

pub struct CliApp {
    mode: CliMode,
    rl: Option<DefaultEditor>,
    commands: HashMap<&'static str, Command>,
    state: CliState,
    theme: ColorfulTheme,
}

pub(crate) struct CliState {
    ledger: Option<Ledger>,
    ledger_path: Option<PathBuf>,
}

impl CliState {
    fn new() -> Self {
        Self {
            ledger: None,
            ledger_path: None,
        }
    }

    fn set_ledger(&mut self, ledger: Ledger) {
        self.ledger = Some(ledger);
    }

    fn set_path(&mut self, path: Option<PathBuf>) {
        self.ledger_path = path;
    }
}

impl CliApp {
    pub fn new(mode: CliMode) -> Result<Self, CliError> {
        let mut commands = HashMap::new();
        for command in build_commands() {
            commands.insert(command.name, command);
        }

        let rl = match mode {
            CliMode::Interactive => Some(DefaultEditor::new()?),
            CliMode::Script => None,
        };

        Ok(Self {
            mode,
            rl,
            commands,
            state: CliState::new(),
            theme: ColorfulTheme::default(),
        })
    }

    pub fn run_loop(&mut self) -> Result<(), CliError> {
        loop {
            let prompt = self.prompt();
            let line = {
                let rl = self
                    .rl
                    .as_mut()
                    .ok_or_else(|| CliError::Internal("interactive editor missing".into()))?;
                rl.readline(&prompt)
            };

            match line {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    if let Some(rl) = self.rl.as_mut() {
                        if let Err(err) = rl.add_history_entry(trimmed) {
                            tracing::debug!("history entry failed: {err:?}");
                        }
                    }

                    match self.process_line(trimmed) {
                        Ok(LoopControl::Continue) => {}
                        Ok(LoopControl::Exit) => break,
                        Err(err) => self.report_error(err)?,
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    if self.confirm_exit()? {
                        break;
                    }
                }
                Err(ReadlineError::Eof) => {
                    println!("{}", "Exiting shell".bright_black());
                    break;
                }
                Err(err) => return Err(err.into()),
            }
        }

        Ok(())
    }

    fn prompt(&self) -> String {
        let context = self
            .state
            .ledger
            .as_ref()
            .map(|ledger| format!("ledger({})", ledger.name))
            .unwrap_or_else(|| "no-ledger".to_string());

        format!("{} {} ", context.bright_cyan(), PROMPT_ARROW.bright_black())
    }

    pub(crate) fn process_line(&mut self, line: &str) -> Result<LoopControl, CommandError> {
        let tokens = match parse_command_line(line) {
            Ok(tokens) => tokens,
            Err(err) => {
                self.print_warning(&format!("{}", err));
                return Ok(LoopControl::Continue);
            }
        };

        if tokens.is_empty() {
            return Ok(LoopControl::Continue);
        }

        let command_name = tokens[0].clone();
        let args_vec: Vec<&str> = tokens.iter().skip(1).map(String::as_str).collect();

        let cmd = command_name.to_lowercase();
        if let Some(command) = self.commands.get(cmd.as_str()) {
            match (command.action)(self, &args_vec) {
                Ok(()) => Ok(LoopControl::Continue),
                Err(CommandError::ExitRequested) => Ok(LoopControl::Exit),
                Err(err) => Err(err),
            }
        } else {
            self.suggest_command(&command_name);
            Ok(LoopControl::Continue)
        }
    }

    fn suggest_command(&self, input: &str) {
        self.print_warning(&format!(
            "Unknown command `{}`. Type `help` to see available commands.",
            input
        ));

        let mut suggestions: Vec<_> = self
            .commands
            .keys()
            .map(|key| (levenshtein(key, input), *key))
            .collect();
        suggestions.sort_by_key(|(distance, _)| *distance);

        if let Some((distance, best)) = suggestions.first() {
            if *distance <= 3 {
                println!("{} {}?", "Did you mean".bright_black(), best.bright_white());
            }
        }
    }

    fn confirm_exit(&self) -> Result<bool, CliError> {
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

    fn report_error(&self, err: CommandError) -> Result<(), CliError> {
        match err {
            CommandError::ExitRequested => Ok(()),
            other => {
                self.print_error(&other.to_string());
                Ok(())
            }
        }
    }

    fn print_error(&self, message: &str) {
        println!("{} {}", "⚠️".bright_yellow(), message.bright_red());
    }

    fn print_warning(&self, message: &str) {
        println!("{} {}", "⚠️".bright_yellow(), message.bright_yellow());
    }

    fn current_ledger(&self) -> Result<&Ledger, CommandError> {
        self.state
            .ledger
            .as_ref()
            .ok_or(CommandError::LedgerNotLoaded)
    }

    fn current_ledger_mut(&mut self) -> Result<&mut Ledger, CommandError> {
        self.state
            .ledger
            .as_mut()
            .ok_or(CommandError::LedgerNotLoaded)
    }

    fn set_ledger(&mut self, ledger: Ledger, path: Option<PathBuf>) {
        self.state.set_ledger(ledger);
        self.state.set_path(path);
    }

    fn command(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    fn command_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.commands.keys().copied().collect();
        names.sort();
        names
    }

    fn run_new_ledger_interactive(&mut self) -> CommandResult {
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
        let ledger = Ledger::new(name, period);
        self.set_ledger(ledger, None);
        println!("{}", "New ledger created".bright_green());
        Ok(())
    }

    fn prompt_budget_period(&self) -> Result<BudgetPeriod, CommandError> {
        let interval = self.prompt_time_interval()?;
        Ok(BudgetPeriod(interval))
    }

    fn prompt_time_interval(&self) -> Result<TimeInterval, CommandError> {
        let options = interval_options();
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Select interval")
            .items(options)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;

        if selection == options.len() - 1 {
            let every: u32 = Input::<u32>::with_theme(&self.theme)
                .with_prompt("Repeat every (number)")
                .validate_with(|value: &u32| -> Result<(), &str> {
                    if *value == 0 {
                        Err("Value must be greater than 0")
                    } else {
                        Ok(())
                    }
                })
                .interact_text()
                .map_err(CommandError::from)?;

            let units = ["Day", "Week", "Month", "Year"];
            let unit_selection = Select::with_theme(&self.theme)
                .with_prompt("Time unit")
                .items(&units)
                .default(2)
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
                "daily" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Day,
                },
                "weekly" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Week,
                },
                "monthly" => TimeInterval {
                    every: 1,
                    unit: TimeUnit::Month,
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

    fn run_new_ledger_script(&mut self, args: &[&str]) -> CommandResult {
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
        let ledger = Ledger::new(name, period);
        self.set_ledger(ledger, None);
        println!("{}", "New ledger created".bright_green());
        Ok(())
    }

    fn load_ledger(&mut self, path: &Path) -> CommandResult {
        let ledger = persistence::load_ledger_from_file(path).map_err(CommandError::from_ledger)?;
        self.set_ledger(ledger, Some(path.to_path_buf()));
        println!("{}", "Ledger loaded".bright_green());
        Ok(())
    }

    fn save_to_path(&mut self, path: &Path) -> CommandResult {
        let ledger = self.current_ledger()?;
        persistence::save_ledger_to_file(ledger, path).map_err(CommandError::from_ledger)?;
        self.state.set_path(Some(path.to_path_buf()));
        println!(
            "{}",
            format!("Ledger saved to {}", path.display()).bright_green()
        );
        Ok(())
    }

    fn add_account_interactive(&mut self) -> CommandResult {
        let name: String = Input::with_theme(&self.theme)
            .with_prompt("Account name")
            .interact_text()
            .map_err(CommandError::from)?;

        let kinds = account_kind_options();
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Account type")
            .items(kinds)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        let kind = parse_account_kind(kinds[selection])?;

        let account = Account::new(name, kind);
        let ledger = self.current_ledger_mut()?;
        ledger.add_account(account);
        println!("{}", "Account added".bright_green());
        Ok(())
    }

    fn add_account_script(&mut self, args: &[&str]) -> CommandResult {
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
        println!("{}", "Account added".bright_green());
        Ok(())
    }

    fn add_category_interactive(&mut self) -> CommandResult {
        let name: String = Input::with_theme(&self.theme)
            .with_prompt("Category name")
            .interact_text()
            .map_err(CommandError::from)?;

        let kinds = category_kind_options();
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Category type")
            .items(kinds)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        let kind = parse_category_kind(kinds[selection])?;

        let category = Category::new(name, kind);
        let ledger = self.current_ledger_mut()?;
        ledger.add_category(category);
        println!("{}", "Category added".bright_green());
        Ok(())
    }

    fn add_category_script(&mut self, args: &[&str]) -> CommandResult {
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
        println!("{}", "Category added".bright_green());
        Ok(())
    }

    fn add_transaction_interactive(&mut self) -> CommandResult {
        {
            let ledger = self.current_ledger()?;
            if ledger.accounts.is_empty() {
                return Err(CommandError::Message(
                    "Add at least one account before creating transactions".into(),
                ));
            }
        }

        let (from, to) = self.select_accounts()?;

        let budgeted_amount: f64 = Input::<f64>::with_theme(&self.theme)
            .with_prompt("Budgeted amount")
            .interact_text()
            .map_err(CommandError::from)?;

        let date_input: String = Input::<String>::with_theme(&self.theme)
            .with_prompt("Scheduled date (YYYY-MM-DD)")
            .validate_with(|input: &String| -> Result<(), &str> {
                NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d")
                    .map(|_| ())
                    .map_err(|_| "Use format YYYY-MM-DD")
            })
            .interact_text()
            .map_err(CommandError::from)?;
        let scheduled_date = NaiveDate::parse_from_str(&date_input, "%Y-%m-%d")
            .map_err(|_| CommandError::InvalidArguments("Invalid date format".into()))?;

        let mut transaction = Transaction::new(from, to, None, scheduled_date, budgeted_amount);
        if Confirm::with_theme(&self.theme)
            .with_prompt("Add recurrence?")
            .default(false)
            .interact()
            .map_err(CommandError::from)?
        {
            let recurrence = self.prompt_recurrence()?;
            transaction.recurrence = Some(recurrence);
        }

        let ledger = self.current_ledger_mut()?;
        ledger.add_transaction(transaction);
        println!("{}", "Transaction added".bright_green());
        Ok(())
    }

    fn add_transaction_script(&mut self, args: &[&str]) -> CommandResult {
        if args.len() < 4 {
            return Err(CommandError::InvalidArguments(
                "usage: add transaction <from_account_index> <to_account_index> <YYYY-MM-DD> <amount>".into(),
            ));
        }

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

        let ledger = self.current_ledger_mut()?;
        if from_index >= ledger.accounts.len() || to_index >= ledger.accounts.len() {
            return Err(CommandError::InvalidArguments(
                "account indices out of range".into(),
            ));
        }
        let from_id = ledger.accounts[from_index].id;
        let to_id = ledger.accounts[to_index].id;
        let transaction = Transaction::new(from_id, to_id, None, date, amount);
        ledger.add_transaction(transaction);
        println!("{}", "Transaction added".bright_green());
        Ok(())
    }

    fn prompt_recurrence(&self) -> Result<Recurrence, CommandError> {
        let interval = self.prompt_time_interval()?;
        let modes = [
            ("Fixed schedule", RecurrenceMode::FixedSchedule),
            ("After last performed", RecurrenceMode::AfterLastPerformed),
        ];
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Recurrence mode")
            .items(&modes.iter().map(|(label, _)| label).collect::<Vec<_>>())
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        Ok(Recurrence {
            interval,
            mode: modes[selection].1.clone(),
        })
    }

    fn select_accounts(&mut self) -> Result<(uuid::Uuid, uuid::Uuid), CommandError> {
        let items: Vec<String> = {
            let ledger = self.current_ledger()?;
            ledger
                .accounts
                .iter()
                .enumerate()
                .map(|(idx, account)| format!("{}: {}", idx, account.name))
                .collect()
        };

        let from_idx = Select::with_theme(&self.theme)
            .with_prompt("From account")
            .items(&items)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        let to_idx = Select::with_theme(&self.theme)
            .with_prompt("To account")
            .items(&items)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;

        let ledger = self.current_ledger()?;
        let from_id = ledger.accounts[from_idx].id;
        let to_id = ledger.accounts[to_idx].id;
        Ok((from_id, to_id))
    }

    fn list_accounts(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        if ledger.accounts.is_empty() {
            println!("{}", "No accounts defined".bright_black());
        } else {
            println!("{}", "Accounts".bright_white().bold());
            for (idx, account) in ledger.accounts.iter().enumerate() {
                println!(
                    "  [{}] {} ({:?})",
                    idx,
                    account.name.bright_white(),
                    account.kind
                );
            }
        }
        Ok(())
    }

    fn list_categories(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        if ledger.categories.is_empty() {
            println!("{}", "No categories defined".bright_black());
        } else {
            println!("{}", "Categories".bright_white().bold());
            for (idx, category) in ledger.categories.iter().enumerate() {
                println!(
                    "  [{}] {} ({:?}){}",
                    idx,
                    category.name.bright_white(),
                    category.kind,
                    category.parent_id.map(|_| " [child]").unwrap_or("")
                );
            }
        }
        Ok(())
    }

    fn list_transactions(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        if ledger.transactions.is_empty() {
            println!("{}", "No transactions recorded".bright_black());
        } else {
            println!("{}", "Transactions".bright_white().bold());
            for (idx, txn) in ledger.transactions.iter().enumerate() {
                println!(
                    "  [{}] {} -> {} | {} | {:.2}",
                    idx, txn.from_account, txn.to_account, txn.scheduled_date, txn.budgeted_amount
                );
            }
        }
        Ok(())
    }

    fn show_summary(&self) -> CommandResult {
        let ledger = self.current_ledger()?;
        println!(
            "{}",
            format!(
                "Ledger `{}`: {} accounts, {} categories, {} transactions",
                ledger.name,
                ledger.accounts.len(),
                ledger.categories.len(),
                ledger.transactions.len()
            )
            .bright_white()
        );
        Ok(())
    }
}

fn build_commands() -> Vec<Command> {
    vec![
        Command::new(
            "help",
            "Show available commands",
            "help [command]",
            cmd_help,
        ),
        Command::new(
            "new-ledger",
            "Create a new ledger",
            "new-ledger [name] [period]",
            cmd_new_ledger,
        ),
        Command::new("load", "Load a ledger from JSON", "load [path]", cmd_load),
        Command::new("save", "Save current ledger", "save [path]", cmd_save),
        Command::new(
            "add",
            "Add an account, category, or transaction",
            "add [account|category|transaction]",
            cmd_add,
        ),
        Command::new(
            "list",
            "List accounts, categories, or transactions",
            "list [accounts|categories|transactions]",
            cmd_list,
        ),
        Command::new("summary", "Show ledger summary", "summary", cmd_summary),
        Command::new("exit", "Exit the shell", "exit", cmd_exit),
    ]
}

fn cmd_help(app: &mut CliApp, args: &[&str]) -> CommandResult {
    if let Some(command) = args.first().map(|name| name.to_lowercase()) {
        if let Some(command) = app.command(&command) {
            println!(
                "{}\n  Usage: {}",
                command.description.bright_white(),
                command.usage.bright_black()
            );
        } else {
            app.suggest_command(args[0]);
        }
        return Ok(());
    }

    println!("{}", "Available commands:".bright_white().bold());
    for name in app.command_names() {
        if let Some(cmd) = app.command(name) {
            println!("  {:<16} {}", name.bright_cyan(), cmd.description);
        }
    }
    println!("Use `help <command>` for details.");
    Ok(())
}

fn cmd_new_ledger(app: &mut CliApp, args: &[&str]) -> CommandResult {
    match app.mode {
        CliMode::Interactive => app.run_new_ledger_interactive(),
        CliMode::Script => app.run_new_ledger_script(args),
    }
}

fn cmd_load(app: &mut CliApp, args: &[&str]) -> CommandResult {
    if let Some(path) = args.first() {
        let path = PathBuf::from(path);
        app.load_ledger(&path)
    } else if app.mode == CliMode::Interactive {
        let path: PathBuf = Input::<String>::with_theme(&app.theme)
            .with_prompt("Path to ledger JSON")
            .interact_text()
            .map(PathBuf::from)
            .map_err(CommandError::from)?;
        app.load_ledger(&path)
    } else {
        Err(CommandError::InvalidArguments("usage: load <path>".into()))
    }
}

fn cmd_save(app: &mut CliApp, args: &[&str]) -> CommandResult {
    if let Some(path) = args.first() {
        let path = PathBuf::from(path);
        app.save_to_path(&path)
    } else if let Some(path) = app.state.ledger_path.clone() {
        app.save_to_path(&path)
    } else if app.mode == CliMode::Interactive {
        let path: PathBuf = Input::<String>::with_theme(&app.theme)
            .with_prompt("Save ledger to")
            .interact_text()
            .map(PathBuf::from)
            .map_err(CommandError::from)?;
        app.save_to_path(&path)
    } else {
        Err(CommandError::InvalidArguments("usage: save <path>".into()))
    }
}

fn cmd_add(app: &mut CliApp, args: &[&str]) -> CommandResult {
    if let Some(target) = args.first() {
        match target.to_lowercase().as_str() {
            "account" => app.add_account_script(&args[1..]),
            "category" => app.add_category_script(&args[1..]),
            "transaction" => app.add_transaction_script(&args[1..]),
            other => Err(CommandError::InvalidArguments(format!(
                "unknown add target `{}`",
                other
            ))),
        }
    } else if app.mode == CliMode::Interactive {
        let options = ["Account", "Category", "Transaction"];
        let choice = Select::with_theme(&app.theme)
            .with_prompt("Add which item?")
            .items(&options)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        match choice {
            0 => app.add_account_interactive(),
            1 => app.add_category_interactive(),
            _ => app.add_transaction_interactive(),
        }
    } else {
        Err(CommandError::InvalidArguments(
            "usage: add <account|category|transaction>".into(),
        ))
    }
}

fn cmd_list(app: &mut CliApp, args: &[&str]) -> CommandResult {
    if let Some(target) = args.first() {
        match target.to_lowercase().as_str() {
            "accounts" => app.list_accounts(),
            "categories" => app.list_categories(),
            "transactions" => app.list_transactions(),
            other => Err(CommandError::InvalidArguments(format!(
                "unknown list target `{}`",
                other
            ))),
        }
    } else if app.mode == CliMode::Interactive {
        let options = ["Accounts", "Categories", "Transactions"];
        let choice = Select::with_theme(&app.theme)
            .with_prompt("List items")
            .items(&options)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        match choice {
            0 => app.list_accounts(),
            1 => app.list_categories(),
            _ => app.list_transactions(),
        }
    } else {
        Err(CommandError::InvalidArguments(
            "usage: list <accounts|categories|transactions>".into(),
        ))
    }
}

fn cmd_summary(app: &mut CliApp, _args: &[&str]) -> CommandResult {
    app.show_summary()
}

fn cmd_exit(_app: &mut CliApp, _args: &[&str]) -> CommandResult {
    Err(CommandError::ExitRequested)
}

fn parse_command_line(line: &str) -> Result<Vec<String>, ParseError> {
    split(line).map_err(|err| ParseError {
        message: err.to_string(),
    })
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

fn account_kind_options() -> &'static [&'static str] {
    &[
        "Bank",
        "Cash",
        "Savings",
        "ExpenseDestination",
        "IncomeSource",
        "Unknown",
    ]
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

fn category_kind_options() -> &'static [&'static str] {
    &["Expense", "Income", "Transfer"]
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

#[derive(Debug)]
struct ParseError {
    message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
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
    fn from_ledger(error: LedgerError) -> Self {
        CommandError::Ledger(error)
    }
}

#[cfg(test)]
pub(crate) fn process_script(lines: &[&str]) -> Result<CliState, CliError> {
    let mut app = CliApp::new(CliMode::Script)?;
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
    use tempfile::NamedTempFile;

    #[test]
    fn parse_line_handles_quotes() {
        let tokens = parse_command_line("new-ledger \"Demo Ledger\" monthly").unwrap();
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
}
