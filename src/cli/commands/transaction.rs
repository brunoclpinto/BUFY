use crate::cli::commands::transaction_handlers;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
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
        "add" => transaction_handlers::handle_add(context, args),
        "edit" => transaction_handlers::handle_edit(context, args),
        "remove" => transaction_handlers::handle_remove(context, args),
        "show" => transaction_handlers::handle_show(context, args),
        "list" => transaction_handlers::handle_list(context),
        "complete" => transaction_handlers::handle_complete(context, args),
        "recurring" => transaction_handlers::handle_recurring(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown transaction subcommand `{}`",
            other
        ))),
    }
}
