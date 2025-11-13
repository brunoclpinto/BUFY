//! Ledger-related CLI commands and helpers.

use std::path::PathBuf;

use chrono::Utc;

use super::simulation;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::cli::menus::{ledger_menu, list_menu, menu_error_to_command_error};
use crate::cli::registry::CommandEntry;
use crate::core::services::SummaryService;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![
        CommandEntry::new(
            "ledger",
            "Ledger operations (new, load, save, backup, restore...)",
            "ledger <new|load|load-ledger|save|save-ledger|backup|list-backups|restore>",
            cmd_ledger,
        ),
        CommandEntry::new(
            "list",
            "List accounts, categories, transactions, simulations, ledgers...",
            "list <accounts|categories|transactions|simulations|ledgers|backups>",
            cmd_list,
        ),
        CommandEntry::new(
            "summary",
            "Show ledger summary",
            "summary [simulation_name] [past|future <n>] | summary custom <start YYYY-MM-DD> <end YYYY-MM-DD>",
            cmd_summary,
        ),
        CommandEntry::new(
            "forecast",
            "Forecast upcoming activity",
            "forecast [simulation_name] [<number> <unit> | custom <start YYYY-MM-DD> <end YYYY-MM-DD>]",
            cmd_forecast,
        ),
    ]
}

fn cmd_ledger(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        return run_ledger_menu(context);
    }

    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: ledger <new|load|load-ledger|save|save-ledger|backup|list-backups|restore>"
                .into(),
        ));
    }

    let (subcommand, rest) = args.split_first().expect("non-empty args");
    dispatch_ledger_action(context, subcommand, rest)
}

fn dispatch_ledger_action(
    context: &mut ShellContext,
    subcommand: &str,
    args: &[&str],
) -> CommandResult {
    match subcommand.to_ascii_lowercase().as_str() {
        "new" => cmd_new_ledger(context, args),
        "load" => cmd_load(context, args),
        "load-ledger" | "load-named" => cmd_load_named(context, args),
        "save" => cmd_save(context, args),
        "save-ledger" | "save-named" => cmd_save_named(context, args),
        "backup" | "backup-ledger" => cmd_backup_ledger(context, args),
        "list-backups" | "backups" => cmd_list_backups(context, args),
        "restore" | "restore-ledger" => cmd_restore_ledger(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown ledger subcommand `{}`. Available: new, load, load-ledger, save, save-ledger, backup, list-backups, restore",
            other
        ))),
    }
}

fn run_ledger_menu(context: &mut ShellContext) -> CommandResult {
    let selection = ledger_menu::show().map_err(menu_error_to_command_error)?;
    let Some(action) = selection else {
        return Ok(());
    };
    match action {
        "new" => cmd_new_ledger(context, &[]),
        "load" => cmd_load(context, &[]),
        "save" => cmd_save(context, &[]),
        "backup" => cmd_backup_ledger(context, &[]),
        "restore" => cmd_restore_ledger(context, &[]),
        "list" => cmd_ledger_overview(context),
        "delete" => cmd_delete_ledger(context),
        _ => Ok(()),
    }
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
        Err(CommandError::InvalidArguments(
            "usage: ledger load <path>".into(),
        ))
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
        Err(CommandError::InvalidArguments(
            "usage: ledger save <path>".into(),
        ))
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
            "usage: ledger save-ledger <name>".into(),
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
            "usage: ledger load-ledger <name>".into(),
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
    if context.mode() == CliMode::Interactive && args.is_empty() {
        return run_list_menu(context);
    }

    if let Some(target) = args.first() {
        execute_list_action(context, target)
    } else {
        Err(CommandError::InvalidArguments(
            "usage: list <accounts|categories|transactions|simulations|ledgers|backups>".into(),
        ))
    }
}

fn run_list_menu(context: &mut ShellContext) -> CommandResult {
    let selection = list_menu::show().map_err(menu_error_to_command_error)?;
    let Some(target) = selection else {
        return Ok(());
    };
    execute_list_action(context, target)
}

fn execute_list_action(context: &mut ShellContext, target: &str) -> CommandResult {
    match target.to_lowercase().as_str() {
        "accounts" => context.list_accounts(),
        "categories" => context.list_categories(),
        "transactions" => context.list_transactions(),
        "simulations" => simulation::list_simulations(context),
        "ledgers" => cmd_ledger_overview(context),
        "backups" => cmd_list_backups(context, &[]),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown list target `{}`",
            other
        ))),
    }
}

fn cmd_restore_ledger(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    match args.len() {
        0 => {
            if !context.can_prompt() {
                return Err(CommandError::InvalidArguments(
                    "usage: ledger restore <backup_reference> [name]".into(),
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
            "usage: ledger restore <backup_reference> [name]".into(),
        )),
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

fn cmd_ledger_overview(context: &mut ShellContext) -> CommandResult {
    if let Some(name) = context.ledger_name() {
        io::print_info(format!("Active ledger: {}", name));
        io::print_info("Listing backups for the active ledger (if any)...");
        cmd_list_backups(context, &[])
    } else {
        io::print_info("No ledger currently loaded. Load or create a ledger to view backups.");
        Ok(())
    }
}

fn cmd_delete_ledger(_context: &mut ShellContext) -> CommandResult {
    io::print_warning("Ledger deletion workflow is not available yet.");
    Ok(())
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
