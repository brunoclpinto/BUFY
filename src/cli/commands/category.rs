use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::registry::CommandEntry;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "category",
        "Manage categories and budgets",
        "category <add|edit|list|budget>",
        cmd_category,
    )]
}

fn cmd_category(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: category <add|edit|list>".into(),
        ));
    }

    match args[0].to_lowercase().as_str() {
        "add" => {
            if context.mode() == CliMode::Interactive && args.len() == 1 {
                context.run_category_add_wizard()
            } else {
                context.add_category_script(&args[1..])
            }
        }
        "edit" => {
            if context.mode() != CliMode::Interactive {
                return Err(CommandError::InvalidArguments(
                    "category edit is only available in interactive mode".into(),
                ));
            }
            let index = if args.len() > 1 {
                args[1].parse::<usize>().map_err(|_| {
                    CommandError::InvalidArguments("category index must be numeric".into())
                })?
            } else {
                match context.select_category_index("Select a category to edit:")? {
                    Some(index) => index,
                    None => return Ok(()),
                }
            };
            context.run_category_edit_wizard(index)
        }
        "list" => context.list_categories(),
        "budget" => cmd_category_budget(context, &args[1..]),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown category subcommand `{}`",
            other
        ))),
    }
}

fn cmd_category_budget(context: &mut ShellContext, args: &[&str]) -> CommandResult {
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
