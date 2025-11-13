use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
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
    let selection = account_menu::show().map_err(menu_error_to_command_error)?;
    let Some(action) = selection else {
        return Ok(());
    };
    dispatch_account_action(context, action, &[])
}

fn dispatch_account_action(
    context: &mut ShellContext,
    action: &str,
    args: &[&str],
) -> CommandResult {
    match action.to_lowercase().as_str() {
        "add" => {
            if context.mode() == CliMode::Interactive && args.is_empty() {
                context.run_account_add_wizard()
            } else {
                context.add_account_script(args)
            }
        }
        "edit" => {
            if context.mode() != CliMode::Interactive {
                return Err(CommandError::InvalidArguments(
                    "account edit is only available in interactive mode".into(),
                ));
            }
            let index = if let Some(value) = args.first() {
                value.parse::<usize>().map_err(|_| {
                    CommandError::InvalidArguments("account index must be numeric".into())
                })?
            } else {
                match context.select_account_index("Select an account to edit:")? {
                    Some(index) => index,
                    None => return Ok(()),
                }
            };
            context.run_account_edit_wizard(index)
        }
        "list" => context.list_accounts(),
        "remove" => {
            io::print_warning("Account removal is not available yet.");
            Ok(())
        }
        "show" => context.list_accounts(),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown account subcommand `{}`",
            other
        ))),
    }
}
