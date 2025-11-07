use std::path::PathBuf;

use chrono::Utc;
use dialoguer::{Input, Select};

use super::CommandDefinition;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::core::services::SummaryService;

pub(crate) fn definitions() -> Vec<CommandDefinition> {
    vec![
        CommandDefinition::new("new-ledger", "Create a new ledger", "new-ledger [name] [period]", cmd_new_ledger),
        CommandDefinition::new("load", "Load a ledger from JSON", "load [path]", cmd_load),
        CommandDefinition::new("load-ledger", "Load a ledger by name from the persistence store", "load-ledger <name>", cmd_load_named),
        CommandDefinition::new("save", "Save current ledger", "save [path]", cmd_save),
        CommandDefinition::new("save-ledger", "Save current ledger by name in the persistence store", "save-ledger [name]", cmd_save_named),
        CommandDefinition::new("backup-ledger", "Create a snapshot of the current ledger", "backup-ledger [name]", cmd_backup_ledger),
        CommandDefinition::new("list-backups", "List available snapshots for the current ledger", "list-backups [name]", cmd_list_backups),
        CommandDefinition::new("restore-ledger", "Restore a ledger from a snapshot", "restore-ledger <backup_index|pattern> [name]", cmd_restore_ledger),
        CommandDefinition::new("add", "Add an account, category, or transaction", "add [account|category|transaction]", cmd_add),
        CommandDefinition::new("list", "List accounts, categories, or transactions", "list [accounts|categories|transactions]", cmd_list),
        CommandDefinition::new("summary", "Show ledger summary", "summary [simulation_name] [past|future <n>] | summary custom <start YYYY-MM-DD> <end YYYY-MM-DD>", cmd_summary),
        CommandDefinition::new("forecast", "Forecast recurring activity", "forecast [simulation_name] [<number> <unit> | custom <start YYYY-MM-DD> <end YYYY-MM-DD>]", cmd_forecast),
    ]
}

fn cmd_new_ledger(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    match context.mode() {
        CliMode::Interactive => context.run_new_ledger_interactive(),
        CliMode::Script => context.run_new_ledger_script(args),
    }
}

fn cmd_load(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if let Some(path) = args.first() {
        let path = PathBuf::from(path);
        context.load_ledger(&path)
    } else if context.mode() == CliMode::Interactive {
        let path: PathBuf = Input::<String>::with_theme(context.theme())
            .with_prompt("Path to ledger JSON")
            .interact_text()
            .map(PathBuf::from)
            .map_err(CommandError::from)?;
        context.load_ledger(&path)
    } else {
        Err(CommandError::InvalidArguments("usage: load <path>".into()))
    }
}

fn cmd_save(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if let Some(path) = args.first() {
        let path = PathBuf::from(path);
        context.save_to_path(&path)
    } else if let Some(name) = context.ledger_name().map(|s| s.to_string()) {
        context.save_named_ledger(&name)
    } else if let Some(path) = context.ledger_path() {
        context.save_to_path(&path)
    } else if context.mode() == CliMode::Interactive {
        let current = context.current_ledger()?;
        let suggested = context
            .ledger_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| current.name.clone());
        let choice = Select::with_theme(context.theme())
            .with_prompt("Choose save method")
            .items(&["Name in store", "Custom path"])
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        if choice == 0 {
            let name: String = Input::<String>::with_theme(context.theme())
                .with_prompt("Ledger name")
                .with_initial_text(suggested)
                .interact_text()
                .map_err(CommandError::from)?;
            context.save_named_ledger(&name)
        } else {
            let path: PathBuf = Input::<String>::with_theme(context.theme())
                .with_prompt("Save ledger to path")
                .interact_text()
                .map(PathBuf::from)
                .map_err(CommandError::from)?;
            context.save_to_path(&path)
        }
    } else {
        Err(CommandError::InvalidArguments("usage: save <path>".into()))
    }
}

fn cmd_save_named(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else if let Some(existing) = context.ledger_name().map(|s| s.to_string()) {
        existing
    } else if context.mode() == CliMode::Interactive {
        Input::<String>::with_theme(context.theme())
            .with_prompt("Ledger name")
            .interact_text()
            .map_err(CommandError::from)?
    } else {
        return Err(CommandError::InvalidArguments(
            "usage: save-ledger <name>".into(),
        ));
    };
    context.save_named_ledger(&name)
}

fn cmd_load_named(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else if context.mode() == CliMode::Interactive {
        Input::<String>::with_theme(context.theme())
            .with_prompt("Ledger name to load")
            .interact_text()
            .map_err(CommandError::from)?
    } else {
        return Err(CommandError::InvalidArguments(
            "usage: load-ledger <name>".into(),
        ));
    };
    context.load_named_ledger(&name)
}

fn cmd_backup_ledger(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        context.require_named_ledger()?.to_string()
    };
    context.create_backup(&name)
}

fn cmd_list(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if let Some(target) = args.first() {
        match target.to_lowercase().as_str() {
            "accounts" => context.list_accounts(),
            "categories" => context.list_categories(),
            "transactions" => context.list_transactions(),
            other => Err(CommandError::InvalidArguments(format!(
                "unknown list target `{}`",
                other
            ))),
        }
    } else if context.mode() == CliMode::Interactive {
        let options = ["Accounts", "Categories", "Transactions"];
        let choice = Select::with_theme(context.theme())
            .with_prompt("List items")
            .items(&options)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        match choice {
            0 => context.list_accounts(),
            1 => context.list_categories(),
            _ => context.list_transactions(),
        }
    } else {
        Err(CommandError::InvalidArguments(
            "usage: list <accounts|categories|transactions>".into(),
        ))
    }
}

fn cmd_restore_ledger(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    match args.len() {
        0 => {
            if !context.can_prompt() {
                return Err(CommandError::InvalidArguments(
                    "usage: restore-ledger <backup_reference> [name]".into(),
                ));
            }
            let name = {
                let named = context.require_named_ledger()?;
                named.to_string()
            };
            let selection = context.select_ledger_backup("Select a backup to restore:")?;
            let Some(path) = selection else {
                io::print_info("Operation cancelled.");
                return Ok(());
            };
            context.restore_backup_from_path(&name, path)
        }
        1 => {
            let reference = args[0];
            let name = context.require_named_ledger()?.to_string();
            context.restore_backup(&name, reference)
        }
        2 => {
            let reference = args[0];
            let name = args[1].to_string();
            context.restore_backup(&name, reference)
        }
        _ => Err(CommandError::InvalidArguments(
            "usage: restore-ledger <backup_reference> [name]".into(),
        )),
    }
}

fn cmd_add(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if let Some(target) = args.first() {
        match target.to_lowercase().as_str() {
            "account" => context.add_account_script(&args[1..]),
            "category" => context.add_category_script(&args[1..]),
            "transaction" => context.add_transaction_script(&args[1..]),
            other => Err(CommandError::InvalidArguments(format!(
                "unknown add target `{}`",
                other
            ))),
        }
    } else if context.mode() == CliMode::Interactive {
        let options = ["Account", "Category", "Transaction"];
        let choice = Select::with_theme(context.theme())
            .with_prompt("Add which item?")
            .items(&options)
            .default(0)
            .interact()
            .map_err(CommandError::from)?;
        match choice {
            0 => context.add_account_interactive(),
            1 => context.add_category_interactive(),
            _ => context.add_transaction_interactive(),
        }
    } else {
        Err(CommandError::InvalidArguments(
            "usage: add <account|category|transaction>".into(),
        ))
    }
}

fn cmd_list_backups(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        context.require_named_ledger()?.to_string()
    };
    context.list_backups(&name)
}

fn cmd_summary(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.show_budget_summary(args)
}

fn cmd_forecast(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let ledger = context.current_ledger()?;
    let today = Utc::now().date_naive();
    let (simulation, remainder) = if !args.is_empty() && ledger.simulation(args[0]).is_some() {
        (Some(args[0]), &args[1..])
    } else {
        (None, args)
    };
    let window = context.resolve_forecast_window(remainder, today)?;
    let report = SummaryService::forecast_window(ledger, window, today, simulation)
        .map_err(CommandError::from)?;
    context.print_forecast_report(ledger, simulation, &report);
    Ok(())
}
