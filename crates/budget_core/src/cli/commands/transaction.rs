pub mod list_transactions;

use chrono::Utc;

use crate::cli::core::{CliMode, CommandError, CommandResult, RecurrenceListFilter, ShellContext};
use crate::cli::io;
use crate::cli::menus::{menu_error_to_command_error, transaction_menu};
use crate::cli::registry::CommandEntry;
use crate::ledger::RecurrenceStatus;
pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "transaction",
        "Manage transactions via wizard flows",
        "transaction <add|edit|remove|show|list|complete|recurring>",
        cmd_transaction,
    )]
}

fn cmd_transaction(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        return run_transaction_menu(context);
    }

    if let Some((subcommand, rest)) = args.split_first() {
        dispatch_transaction_action(context, subcommand, rest)
    } else {
        Err(CommandError::InvalidArguments(
            "usage: transaction <add|edit|remove|show|list|complete|recurring>".into(),
        ))
    }
}

fn run_transaction_menu(context: &mut ShellContext) -> CommandResult {
    let selection = transaction_menu::show(context).map_err(menu_error_to_command_error)?;
    let Some(action) = selection else {
        return Ok(());
    };
    dispatch_transaction_action(context, action.as_str(), &[])
}

fn dispatch_transaction_action(
    context: &mut ShellContext,
    subcommand: &str,
    args: &[&str],
) -> CommandResult {
    match subcommand.to_ascii_lowercase().as_str() {
        "add" => handle_add(context, args),
        "edit" => handle_edit(context, args),
        "remove" => handle_remove(context, args),
        "show" => handle_show(context, args),
        "list" => handle_list(context),
        "complete" => handle_complete(context, args),
        "recurring" => handle_recurring(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown transaction subcommand `{}`",
            other
        ))),
    }
}

fn handle_add(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_add(args)
}

fn handle_edit(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_edit(args)
}

fn handle_remove(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_remove(args)
}

fn handle_show(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_show(args)
}

fn handle_list(context: &mut ShellContext) -> CommandResult {
    list_transactions::run_list_transactions(context)
}

fn handle_complete(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_complete(args)
}

fn handle_recurring(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if args.is_empty() {
        return context.list_recurrences(RecurrenceListFilter::All);
    }
    match args[0].to_lowercase().as_str() {
        "list" => {
            let filter = RecurrenceListFilter::parse(args.get(1).copied())?;
            context.list_recurrences(filter)
        }
        "edit" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: transaction recurring edit <transaction_index>",
                "Select a transaction to edit recurrence:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            context.recurrence_edit(idx)
        }
        "clear" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: transaction recurring clear <transaction_index>",
                "Select a transaction to clear recurrence:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            context.recurrence_clear(idx)
        }
        "pause" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: transaction recurring pause <transaction_index>",
                "Select a transaction to pause recurrence:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            context.recurrence_set_status(idx, RecurrenceStatus::Paused)
        }
        "resume" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: transaction recurring resume <transaction_index>",
                "Select a transaction to resume recurrence:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            context.recurrence_set_status(idx, RecurrenceStatus::Active)
        }
        "skip" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: transaction recurring skip <transaction_index> <YYYY-MM-DD>",
                "Select a transaction to skip a scheduled date:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            let date = if args.len() > 2 {
                crate::cli::core::parse_date(args[2])?
            } else if context.mode() == CliMode::Interactive {
                let response = io::prompt_text("Date to skip (YYYY-MM-DD)", None)
                    .map_err(CommandError::from)?;
                let Some(input) = response else {
                    io::print_info("Operation cancelled.");
                    return Ok(());
                };
                crate::cli::core::parse_date(input.trim())?
            } else {
                return Err(CommandError::InvalidArguments(
                    "usage: transaction recurring skip <transaction_index> <YYYY-MM-DD>".into(),
                ));
            };
            context.recurrence_skip_date(idx, date)
        }
        "sync" => {
            let reference = if args.len() > 1 {
                crate::cli::core::parse_date(args[1])?
            } else {
                Utc::now().date_naive()
            };
            context.recurrence_sync(reference)
        }
        other => Err(CommandError::InvalidArguments(format!(
            "unknown transaction recurring subcommand `{}`",
            other
        ))),
    }
}
