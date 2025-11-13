use crate::cli::commands::simulation_handlers;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::menus::{menu_error_to_command_error, simulation_menu};
use crate::cli::registry::CommandEntry;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![CommandEntry::new(
        "simulation",
        "Manage simulations and what-if scenarios",
        "simulation <list|create|enter|leave|apply|discard|changes|add|modify|exclude>",
        cmd_simulation,
    )]
}

fn cmd_simulation(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if context.mode() == CliMode::Interactive && args.is_empty() {
        let selection = simulation_menu::show(context).map_err(menu_error_to_command_error)?;
        if let Some(action) = selection {
            return dispatch_action(context, action.as_str(), &[]);
        }
        return Ok(());
    }

    if let Some((subcommand, rest)) = args.split_first() {
        dispatch_action(context, subcommand, rest)
    } else {
        Err(CommandError::InvalidArguments(
            "usage: simulation <list|create|enter|leave|apply|discard|changes|add|modify|exclude>"
                .into(),
        ))
    }
}

fn dispatch_action(context: &mut ShellContext, action: &str, args: &[&str]) -> CommandResult {
    match action.to_ascii_lowercase().as_str() {
        "list" | "ls" => simulation_handlers::list_simulations(context),
        "create" | "new" => simulation_handlers::handle_create(context, args),
        "enter" => simulation_handlers::handle_enter(context, args),
        "leave" => simulation_handlers::handle_leave(context),
        "apply" => simulation_handlers::handle_apply(context, args),
        "discard" => simulation_handlers::handle_discard(context, args),
        "changes" | "show" => {
            simulation_handlers::handle_workflow_action(context, "changes", args)
        }
        "add" => simulation_handlers::handle_workflow_action(context, "add", args),
        "modify" => simulation_handlers::handle_workflow_action(context, "modify", args),
        "exclude" => simulation_handlers::handle_workflow_action(context, "exclude", args),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown simulation subcommand `{}`. Available: list, create, enter, leave, apply, discard, changes, add, modify, exclude",
            other
        ))),
    }
}
