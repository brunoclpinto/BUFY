use chrono::Utc;

use crate::cli::core::{CliMode, CommandError, CommandResult, RecurrenceListFilter, ShellContext};
use crate::cli::io;
use crate::ledger::RecurrenceStatus;

pub fn handle_add(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_add(args)
}

pub fn handle_edit(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_edit(args)
}

pub fn handle_remove(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_remove(args)
}

pub fn handle_show(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_show(args)
}

pub fn handle_list(context: &mut ShellContext) -> CommandResult {
    context.list_transactions()
}

pub fn handle_complete(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    context.transaction_complete(args)
}

pub fn handle_recurring(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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
                let input = io::prompt_text("Date to skip (YYYY-MM-DD)", None)
                    .map_err(CommandError::from)?;
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
