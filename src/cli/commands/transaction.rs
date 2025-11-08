use chrono::Utc;

use super::CommandDefinition;
use crate::cli::core::{CliMode, CommandError, CommandResult, RecurrenceListFilter, ShellContext};
use crate::cli::io;
pub(crate) fn definitions() -> Vec<CommandDefinition> {
    vec![
        CommandDefinition::new(
            "transaction",
            "Manage transactions via wizard flows",
            "transaction <add|edit|remove|show|complete>",
            cmd_transaction,
        ),
        CommandDefinition::new(
            "complete",
            "Mark a transaction as completed",
            "complete <transaction_index> <YYYY-MM-DD> <amount>",
            cmd_complete,
        ),
        CommandDefinition::new(
            "recurring",
            "Manage recurring schedules",
            "recurring [list|edit|clear|pause|resume|skip|sync] ...",
            cmd_recurring,
        ),
    ]
}

fn cmd_transaction(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if let Some((subcommand, rest)) = args.split_first() {
        match subcommand.to_ascii_lowercase().as_str() {
            "add" => context.transaction_add(rest),
            "edit" => context.transaction_edit(rest),
            "remove" => context.transaction_remove(rest),
            "show" => context.transaction_show(rest),
            "complete" => context.transaction_complete(rest),
            other => Err(CommandError::InvalidArguments(format!(
                "unknown transaction subcommand `{}`",
                other
            ))),
        }
    } else {
        Err(CommandError::InvalidArguments(
            "usage: transaction <add|edit|remove|show|complete>".into(),
        ))
    }
}

fn cmd_complete(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.legacy_complete(args)
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
