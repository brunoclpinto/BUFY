use std::path::PathBuf;

use chrono::Utc;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::core::services::SummaryService;

pub fn handle_new(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    match context.mode() {
        CliMode::Interactive => context.run_new_ledger_interactive(),
        CliMode::Script => context.run_new_ledger_script(args),
    }
}

pub fn handle_load(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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

pub fn handle_load_named(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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

pub fn handle_save(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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

pub fn handle_save_named(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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

pub fn handle_backup(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        context.require_named_ledger()?
    };
    context.create_backup(&name)
}

pub fn handle_list_backups(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        context.require_named_ledger()?
    };
    context.list_backups(&name)
}

pub fn handle_restore(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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

pub fn handle_overview(context: &mut ShellContext) -> CommandResult {
    if let Some(name) = context.ledger_name() {
        io::print_info(format!("Active ledger: {}", name));
        io::print_info("Listing backups for the active ledger (if any)...");
        handle_list_backups(context, &[])
    } else {
        io::print_info("No ledger currently loaded. Load or create a ledger to view backups.");
        Ok(())
    }
}

pub fn handle_delete(_context: &mut ShellContext) -> CommandResult {
    io::print_warning("Ledger deletion workflow is not available yet.");
    Ok(())
}

pub fn handle_summary(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.show_budget_summary(args)
}

pub fn handle_forecast(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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
