use crate::cli::commands::category_handlers;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
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
        "add" => category_handlers::handle_add(context, args),
        "edit" => category_handlers::handle_edit(context, args),
        "list" => category_handlers::handle_list(context),
        "show" => category_handlers::handle_show(context),
        "remove" => category_handlers::handle_remove(context),
        "budget" => category_handlers::handle_budget(context, args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown category subcommand `{}`",
            other
        ))),
    }
}
