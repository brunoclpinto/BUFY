use super::CommandDefinition;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};

pub(crate) fn definitions() -> Vec<CommandDefinition> {
    vec![CommandDefinition::new(
        "account",
        "Manage accounts via wizard flows",
        "account <add|edit|list>",
        cmd_account,
    )]
}

fn cmd_account(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: account <add|edit|list>".into(),
        ));
    }

    match args[0].to_lowercase().as_str() {
        "add" => {
            if context.mode() == CliMode::Interactive && args.len() == 1 {
                context.run_account_add_wizard()
            } else {
                context.add_account_script(&args[1..])
            }
        }
        "edit" => {
            if context.mode() != CliMode::Interactive {
                return Err(CommandError::InvalidArguments(
                    "account edit is only available in interactive mode".into(),
                ));
            }
            let index = if args.len() > 1 {
                args[1].parse::<usize>().map_err(|_| {
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
        other => Err(CommandError::InvalidArguments(format!(
            "unknown account subcommand `{}`",
            other
        ))),
    }
}
