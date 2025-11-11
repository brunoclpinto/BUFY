//! Ledger-related CLI commands and helpers.

use std::path::PathBuf;

use chrono::Utc;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::cli::registry::CommandEntry;
use crate::core::services::SummaryService;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![
        CommandEntry::new("new-ledger", "Create a new ledger", "new-ledger [name] [period]", cmd_new_ledger),
        CommandEntry::new("load", "Load a ledger from JSON", "load [path]", cmd_load),
        CommandEntry::new("load-ledger", "Load a ledger by name from the persistence store", "load-ledger <name>", cmd_load_named),
        CommandEntry::new("save", "Save current ledger", "save [path]", cmd_save),
        CommandEntry::new("save-ledger", "Save current ledger by name in the persistence store", "save-ledger [name]", cmd_save_named),
        CommandEntry::new("backup-ledger", "Create a snapshot of the current ledger", "backup-ledger [name]", cmd_backup_ledger),
        CommandEntry::new("list-backups", "List available snapshots for the current ledger", "list-backups [name]", cmd_list_backups),
        CommandEntry::new("restore-ledger", "Restore a ledger from a snapshot", "restore-ledger <backup_index|pattern> [name]", cmd_restore_ledger),
        CommandEntry::new("add", "Add an account, category, or transaction", "add [account|category|transaction]", cmd_add),
        CommandEntry::new("list", "List accounts, categories, or transactions", "list [accounts|categories|transactions]", cmd_list),
        CommandEntry::new("summary", "Show ledger summary", "summary [simulation_name] [past|future <n>] | summary custom <start YYYY-MM-DD> <end YYYY-MM-DD>", cmd_summary),
        CommandEntry::new("forecast", "Forecast recurring activity", "forecast [simulation_name] [<number> <unit> | custom <start YYYY-MM-DD> <end YYYY-MM-DD>]", cmd_forecast),
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
        let response = io::prompt_text("Path to ledger JSON", None).map_err(CommandError::from)?;
        context.load_ledger(&PathBuf::from(response.trim()))
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
        let suggested = if let Some(name) = context.ledger_name() {
            name
        } else {
            context.with_ledger(|ledger| Ok(ledger.name.clone()))?
        };
        let choice =
            io::prompt_select_index("Choose save method", &["Name in store", "Custom path"])
                .map_err(CommandError::from)?;
        if choice == 0 {
            let name = io::prompt_text("Ledger name", Some(suggested.as_str()))
                .map_err(CommandError::from)?;
            context.save_named_ledger(&name)
        } else {
            let path = io::prompt_text("Save ledger to path", None).map_err(CommandError::from)?;
            context.save_to_path(&PathBuf::from(path.trim()))
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
        io::prompt_text("Ledger name", None).map_err(CommandError::from)?
    } else {
        return Err(CommandError::InvalidArguments(
            "usage: save-ledger <name>".into(),
        ));
    };
    let name = name.trim().to_string();
    context.save_named_ledger(&name)
}

fn cmd_load_named(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else if context.mode() == CliMode::Interactive {
        io::prompt_text("Ledger name to load", None).map_err(CommandError::from)?
    } else {
        return Err(CommandError::InvalidArguments(
            "usage: load-ledger <name>".into(),
        ));
    };
    let name = name.trim().to_string();
    context.load_named_ledger(&name)
}

fn cmd_backup_ledger(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        context.require_named_ledger()?
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
        let choice = io::prompt_select_index("List items", &options).map_err(CommandError::from)?;
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
            let Some(backup_name) = selection else {
                io::print_info("Operation cancelled.");
                return Ok(());
            };
            context.restore_backup_from_name(&name, backup_name)
        }
        1 => {
            let reference = args[0];
            let name = context.require_named_ledger()?;
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
        let choice =
            io::prompt_select_index("Add which item?", &options).map_err(CommandError::from)?;
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
        context.require_named_ledger()?
    };
    context.list_backups(&name)
}

fn cmd_summary(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.show_budget_summary(args)
}

fn cmd_forecast(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.with_ledger(|ledger| {
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
    })
}
