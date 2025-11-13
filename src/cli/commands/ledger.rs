//! Root ledger command plus list/summary/forecast entry points.

use crate::cli::commands::{ledger_handlers, list_handlers};
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::menus::{ledger_menu, list_menu, menu_error_to_command_error};
use crate::cli::registry::CommandEntry;

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
    dispatch_action(context, subcommand, rest)
}

fn dispatch_action(context: &mut ShellContext, subcommand: &str, args: &[&str]) -> CommandResult {
    match subcommand.to_ascii_lowercase().as_str() {
        "new" => ledger_handlers::handle_new(context, args),
        "load" => ledger_handlers::handle_load(context, args),
        "load-ledger" | "load-named" => ledger_handlers::handle_load_named(context, args),
        "save" => ledger_handlers::handle_save(context, args),
        "save-ledger" | "save-named" => ledger_handlers::handle_save_named(context, args),
        "backup" | "backup-ledger" => ledger_handlers::handle_backup(context, args),
        "list-backups" | "backups" => ledger_handlers::handle_list_backups(context, args),
        "restore" | "restore-ledger" => ledger_handlers::handle_restore(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown ledger subcommand `{}`. Available: new, load, load-ledger, save, save-ledger, backup, list-backups, restore",
            other
        ))),
    }
}

fn run_ledger_menu(context: &mut ShellContext) -> CommandResult {
    let selection = ledger_menu::show(context).map_err(menu_error_to_command_error)?;
    if let Some(action) = selection {
        match action.as_str() {
            "new" => ledger_handlers::handle_new(context, &[]),
            "load" => ledger_handlers::handle_load(context, &[]),
            "save" => ledger_handlers::handle_save(context, &[]),
            "backup" => ledger_handlers::handle_backup(context, &[]),
            "restore" => ledger_handlers::handle_restore(context, &[]),
            "list" => ledger_handlers::handle_overview(context),
            "delete" => ledger_handlers::handle_delete(context),
            _ => Ok(()),
        }
    } else {
        Ok(())
    }
}

fn cmd_list(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        let selection = list_menu::show(context).map_err(menu_error_to_command_error)?;
        if let Some(action) = selection {
            list_handlers::dispatch(context, action.as_str())
        } else {
            Ok(())
        }
    } else if let Some(target) = args.first() {
        list_handlers::dispatch(context, target)
    } else {
        Err(CommandError::InvalidArguments(
            "usage: list <accounts|categories|transactions|simulations|ledgers|backups>".into(),
        ))
    }
}

fn cmd_summary(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    ledger_handlers::handle_summary(context, args)
}

fn cmd_forecast(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    ledger_handlers::handle_forecast(context, args)
}
