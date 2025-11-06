use super::CommandDefinition;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};

pub(crate) fn definitions() -> Vec<CommandDefinition> {
    vec![CommandDefinition::new(
        "category",
        "Manage categories via wizard flows",
        "category <add|edit|list>",
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
        other => Err(CommandError::InvalidArguments(format!(
            "unknown category subcommand `{}`",
            other
        ))),
    }
}
