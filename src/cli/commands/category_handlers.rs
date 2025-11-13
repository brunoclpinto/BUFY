use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;

pub fn handle_add(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        context.run_category_add_wizard()
    } else {
        context.add_category_script(args)
    }
}

pub fn handle_edit(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() != CliMode::Interactive {
        return Err(CommandError::InvalidArguments(
            "category edit is only available in interactive mode".into(),
        ));
    }
    let index = if let Some(value) = args.first() {
        value
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidArguments("category index must be numeric".into()))?
    } else {
        match context.select_category_index("Select a category to edit:")? {
            Some(index) => index,
            None => return Ok(()),
        }
    };
    context.run_category_edit_wizard(index)
}

pub fn handle_list(context: &mut ShellContext) -> CommandResult {
    context.list_categories()
}

pub fn handle_show(context: &mut ShellContext) -> CommandResult {
    context.list_categories()
}

pub fn handle_remove(_context: &mut ShellContext) -> CommandResult {
    io::print_warning("Category removal is not available yet.");
    Ok(())
}

pub fn handle_budget(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: category budget <set|show|clear> ...".into(),
        ));
    }
    match args[0].to_lowercase().as_str() {
        "set" => context.category_budget_set(&args[1..]),
        "show" => context.category_budget_show(&args[1..]),
        "clear" => context.category_budget_clear(&args[1..]),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown category budget action `{}`",
            other
        ))),
    }
}
