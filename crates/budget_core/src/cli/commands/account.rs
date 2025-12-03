pub mod list_accounts;

use crate::cli::commands::account_handlers;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::menus::{account_menu, menu_error_to_command_error};
use crate::cli::registry::CommandEntry;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "account",
        "Manage accounts via wizard flows",
        "account <add|edit|list|remove|show>",
        cmd_account,
    )]
}

fn cmd_account(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        return run_account_menu(context);
    }

    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: account <add|edit|list|remove|show>".into(),
        ));
    }

    dispatch_account_action(context, args[0], &args[1..])
}

fn run_account_menu(context: &mut ShellContext) -> CommandResult {
    let selection = account_menu::show(context).map_err(menu_error_to_command_error)?;
    let Some(action) = selection else {
        return Ok(());
    };
    dispatch_account_action(context, action.as_str(), &[])
}

fn dispatch_account_action(
    context: &mut ShellContext,
    action: &str,
    args: &[&str],
) -> CommandResult {
    match action.to_lowercase().as_str() {
        "add" => account_handlers::handle_add(context, args),
        "edit" => account_handlers::handle_edit(context, args),
        "list" => account_handlers::handle_list(context),
        "remove" => account_handlers::handle_remove(context),
        "show" => account_handlers::handle_show(context),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown account subcommand `{}`",
            other
        ))),
    }
}
