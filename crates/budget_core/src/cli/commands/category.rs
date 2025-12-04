pub mod list_categories;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::cli::menus::{category_menu, menu_error_to_command_error};
use crate::cli::registry::CommandEntry;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "category",
        "Manage categories and budgets",
        "category <add|edit|list|remove|show|budget>",
        cmd_category,
    )]
}

fn cmd_category(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        return run_category_menu(context);
    }

    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: category <add|edit|list|remove|show|budget>".into(),
        ));
    }

    dispatch_category_action(context, args[0], &args[1..])
}

fn run_category_menu(context: &mut ShellContext) -> CommandResult {
    let selection = category_menu::show(context).map_err(menu_error_to_command_error)?;
    let Some(action) = selection else {
        return Ok(());
    };
    dispatch_category_action(context, action.as_str(), &[])
}

fn dispatch_category_action(
    context: &mut ShellContext,
    action: &str,
    args: &[&str],
) -> CommandResult {
    match action.to_lowercase().as_str() {
        "add" => handle_add(context, args),
        "edit" => handle_edit(context, args),
        "list" => handle_list(context),
        "show" => handle_show(context),
        "remove" => handle_remove(context),
        "budget" => handle_budget(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown category subcommand `{}`",
            other
        ))),
    }
}

fn handle_add(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        context.run_category_add_wizard()
    } else {
        context.add_category_script(args)
    }
}

fn handle_edit(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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

fn handle_list(context: &mut ShellContext) -> CommandResult {
    list_categories::run_list_categories(context)
}

fn handle_show(context: &mut ShellContext) -> CommandResult {
    list_categories::run_list_categories(context)
}

fn handle_remove(_context: &mut ShellContext) -> CommandResult {
    io::print_warning("Category removal is not available yet.");
    Ok(())
}

fn handle_budget(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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
