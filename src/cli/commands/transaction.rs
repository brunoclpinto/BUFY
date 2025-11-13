use chrono::Utc;

use crate::cli::core::{CliMode, CommandError, CommandResult, RecurrenceListFilter, ShellContext};
use crate::cli::io;
use crate::cli::menus::{menu_error_to_command_error, transaction_menu};
use crate::cli::registry::CommandEntry;
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
    let selection = transaction_menu::show().map_err(menu_error_to_command_error)?;
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
        "add" => context.transaction_add(args),
        "edit" => context.transaction_edit(args),
        "remove" => context.transaction_remove(args),
        "show" => context.transaction_show(args),
        "list" => context.list_transactions(),
        "complete" => context.transaction_complete(args),
        "recurring" => cmd_recurring(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown transaction subcommand `{}`",
            other
        ))),
    }
}

fn cmd_recurring(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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
                "usage: recurring edit <transaction_index>",
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
                "usage: recurring clear <transaction_index>",
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
                "usage: recurring pause <transaction_index>",
                "Select a transaction to pause recurrence:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            context.recurrence_set_status(idx, crate::ledger::RecurrenceStatus::Paused)
        }
        "resume" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: recurring resume <transaction_index>",
                "Select a transaction to resume recurrence:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            context.recurrence_set_status(idx, crate::ledger::RecurrenceStatus::Active)
        }
        "skip" => {
            let idx = match context.transaction_index_from_arg(
                args.get(1).copied(),
                "usage: recurring skip <transaction_index> <YYYY-MM-DD>",
                "Select a transaction to skip a scheduled date:",
            )? {
                Some(idx) => idx,
                None => return Ok(()),
            };
            let date = if args.len() > 2 {
                crate::cli::core::parse_date(args[2])?
            } else if context.mode() == CliMode::Interactive {
                let input = io::prompt_text("Date to skip (YYYY-MM-DD)", None)
                    .map_err(CommandError::from)?;
                crate::cli::core::parse_date(input.trim())?
            } else {
                return Err(CommandError::InvalidArguments(
                    "usage: recurring skip <transaction_index> <YYYY-MM-DD>".into(),
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
            "unknown recurring subcommand `{}`",
            other
        ))),
    }
}
