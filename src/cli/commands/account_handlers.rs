use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;

pub fn handle_add(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        context.run_account_add_wizard()
    } else {
        context.add_account_script(args)
    }
}

pub fn handle_edit(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() != CliMode::Interactive {
        return Err(CommandError::InvalidArguments(
            "account edit is only available in interactive mode".into(),
        ));
    }
    let index = if let Some(value) = args.first() {
        value
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidArguments("account index must be numeric".into()))?
    } else {
        match context.select_account_index("Select an account to edit:")? {
            Some(index) => index,
            None => return Ok(()),
        }
    };
    context.run_account_edit_wizard(index)
}

pub fn handle_list(context: &mut ShellContext) -> CommandResult {
    context.list_accounts()
}

pub fn handle_show(context: &mut ShellContext) -> CommandResult {
    context.list_accounts()
}

pub fn handle_remove(_context: &mut ShellContext) -> CommandResult {
    io::print_warning("Account removal is not available yet.");
    Ok(())
}
