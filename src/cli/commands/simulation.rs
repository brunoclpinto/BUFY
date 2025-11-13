//! CLI command handlers for budgeting simulations.

use chrono::Local;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::cli::menus::{menu_error_to_command_error, simulation_menu};
use crate::cli::output::section as output_section;
use crate::cli::registry::CommandEntry;
use crate::ledger::SimulationStatus;

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
        return run_simulation_menu(context);
    }

    if let Some((subcommand, rest)) = args.split_first() {
        dispatch_simulation_action(context, subcommand, rest)
    } else {
        Err(CommandError::InvalidArguments(
            "usage: simulation <list|create|enter|leave|apply|discard|changes|add|modify|exclude>"
                .into(),
        ))
    }
}

fn run_simulation_menu(context: &mut ShellContext) -> CommandResult {
    let selection = simulation_menu::show().map_err(menu_error_to_command_error)?;
    let Some(action) = selection else {
        return Ok(());
    };
    dispatch_simulation_action(context, action, &[])
}

fn dispatch_simulation_action(
    context: &mut ShellContext,
    action: &str,
    args: &[&str],
) -> CommandResult {
    match action.to_ascii_lowercase().as_str() {
        "list" | "ls" => list_simulations(context),
        "create" | "new" => cmd_create_simulation(context, args),
        "enter" => cmd_enter_simulation(context, args),
        "leave" => cmd_leave_simulation(context, args),
        "apply" => cmd_apply_simulation(context, args),
        "discard" => cmd_discard_simulation(context, args),
        "show" => {
            let delegated = ["changes"];
            cmd_simulation_workflow(context, &delegated)
        }
        "changes" | "add" | "modify" | "exclude" => {
            let mut forwarded: Vec<&str> = Vec::with_capacity(args.len() + 1);
            forwarded.push(action);
            forwarded.extend_from_slice(args);
            cmd_simulation_workflow(context, &forwarded)
        }
        other => Err(CommandError::InvalidArguments(format!(
            "unknown simulation subcommand `{}`. Available: list, create, enter, leave, apply, discard, changes, add, modify, exclude",
            other
        ))),
    }
}

pub(crate) fn list_simulations(context: &mut ShellContext) -> CommandResult {
    context.with_ledger(|ledger| {
        let sims = ledger.simulations();
        if sims.is_empty() {
            io::print_warning("No simulations defined.");
            return Ok(());
        }
        output_section("Simulations");
        for sim in sims {
            io::print_info(format!(
                "  {:<20} {:<10} changes:{:>2} updated:{}",
                sim.name,
                format!("{:?}", sim.status),
                sim.changes.len(),
                sim.updated_at
            ));
        }
        Ok(())
    })
}

fn cmd_create_simulation(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        loop {
            let value = io::prompt_text("Simulation name", None).map_err(CommandError::from)?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                io::print_error("Name cannot be empty.");
                continue;
            }
            break trimmed.to_string();
        }
    };
    let notes: Option<String> = if context.mode() == CliMode::Interactive {
        let text = io::prompt_text("Notes (optional)", None).map_err(CommandError::from)?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    } else {
        None
    };
    context.with_ledger_mut(|ledger| {
        ledger
            .create_simulation(name.clone(), notes.clone())
            .map(|_| ())
            .map_err(CommandError::from_core)
    })?;
    io::print_success(format!("Simulation `{}` created.", name));
    Ok(())
}

fn cmd_enter_simulation(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = match context.resolve_simulation_name(
        args.first().copied(),
        "Select a simulation to enter:",
        false,
        "usage: simulation enter <name>",
    )? {
        Some(name) => name,
        None => {
            io::print_info("Operation cancelled.");
            return Ok(());
        }
    };
    let (canonical, created) = context.with_ledger(|ledger| {
        let sim = ledger.simulation(&name).ok_or_else(|| {
            CommandError::InvalidArguments(format!("simulation `{}` not found", name))
        })?;
        if sim.status != SimulationStatus::Pending {
            return Err(CommandError::InvalidArguments(format!(
                "simulation `{}` is not editable",
                name
            )));
        }
        Ok((sim.name.clone(), sim.created_at))
    })?;
    context.set_active_simulation(Some(canonical.clone()));
    let created = created.with_timezone(&Local);
    io::print_success(format!(
        "Entered simulation `{}` (Created: {})",
        canonical,
        created.format("%Y-%m-%d %H:%M")
    ));
    Ok(())
}

fn cmd_leave_simulation(context: &mut ShellContext, _args: &[&str]) -> CommandResult {
    if context.active_simulation_name().is_none() {
        return Err(CommandError::InvalidArguments(
            "No active simulation to leave".into(),
        ));
    }
    context.clear_active_simulation();
    io::print_success("Simulation mode cleared.");
    Ok(())
}

fn cmd_apply_simulation(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = match context.resolve_simulation_name(
        args.first().copied(),
        "Select a simulation to apply:",
        false,
        "usage: simulation apply <name>",
    )? {
        Some(name) => name,
        None => {
            io::print_info("Operation cancelled.");
            return Ok(());
        }
    };
    let created = context.with_ledger(|ledger| {
        ledger
            .simulation(&name)
            .map(|sim| sim.created_at)
            .ok_or_else(|| {
                CommandError::InvalidArguments(format!("simulation `{}` not found", name))
            })
    })?;
    context.with_ledger_mut(|ledger| {
        ledger
            .apply_simulation(&name)
            .map_err(CommandError::from_core)
    })?;
    if context
        .active_simulation_name()
        .map(|active| active.eq_ignore_ascii_case(&name))
        .unwrap_or(false)
    {
        context.clear_active_simulation();
    }
    let created_local = created.with_timezone(&Local);
    io::print_success(format!(
        "Simulation `{}` applied (Created: {})",
        name,
        created_local.format("%Y-%m-%d %H:%M")
    ));
    Ok(())
}

fn cmd_discard_simulation(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = match context.resolve_simulation_name(
        args.first().copied(),
        "Select a simulation to discard:",
        false,
        "usage: simulation discard <name>",
    )? {
        Some(name) => name,
        None => {
            io::print_info("Operation cancelled.");
            return Ok(());
        }
    };
    let (created, was_active) = context.with_ledger(|ledger| {
        let sim = ledger.simulation(&name).ok_or_else(|| {
            CommandError::InvalidArguments(format!("simulation `{}` not found", name))
        })?;
        Ok((
            Some(sim.created_at),
            context
                .active_simulation_name()
                .map(|active| active.eq_ignore_ascii_case(&name))
                .unwrap_or(false),
        ))
    })?;
    if context.mode() == CliMode::Interactive {
        let confirm = io::confirm_action(&format!("Discard simulation `{}`?", name))
            .map_err(CommandError::from)?;
        if !confirm {
            io::print_info("Operation cancelled.");
            return Ok(());
        }
    }
    context.with_ledger_mut(|ledger| {
        ledger
            .discard_simulation(&name)
            .map_err(CommandError::from_core)
    })?;
    if was_active {
        context.clear_active_simulation();
    }
    let summary = created
        .map(|ts| {
            let local_ts = ts.with_timezone(&Local);
            format!(
                "Simulation `{}` discarded (Created: {})",
                name,
                local_ts.format("%Y-%m-%d %H:%M")
            )
        })
        .unwrap_or_else(|| format!("Simulation `{}` discarded", name));
    io::print_success(summary);
    Ok(())
}

fn cmd_simulation_workflow(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if args.is_empty() {
        return Err(CommandError::InvalidArguments(
            "usage: simulation <changes|add|modify|exclude> [simulation_name]".into(),
        ));
    }
    let sub = args[0].to_lowercase();
    let target_name = args.get(1).copied();
    match sub.as_str() {
        "changes" | "show" => {
            let name = match context.resolve_simulation_name(
                target_name,
                "Select a simulation to inspect:",
                true,
                "usage: simulation changes [simulation_name]",
            )? {
                Some(name) => name,
                None => {
                    io::print_info("Operation cancelled.");
                    return Ok(());
                }
            };
            context.print_simulation_changes(&name)
        }
        "add" => {
            let name = match context.resolve_simulation_name(
                target_name,
                "Select a simulation to add a transaction to:",
                true,
                "usage: simulation add [simulation_name]",
            )? {
                Some(name) => name,
                None => {
                    io::print_info("Operation cancelled.");
                    return Ok(());
                }
            };
            context.simulation_add_transaction(&name)
        }
        "exclude" => {
            let name = match context.resolve_simulation_name(
                target_name,
                "Select a simulation to exclude a transaction from:",
                true,
                "usage: simulation exclude [simulation_name]",
            )? {
                Some(name) => name,
                None => {
                    io::print_info("Operation cancelled.");
                    return Ok(());
                }
            };
            context.simulation_exclude_transaction(&name)
        }
        "modify" => {
            let name = match context.resolve_simulation_name(
                target_name,
                "Select a simulation to modify:",
                true,
                "usage: simulation modify [simulation_name]",
            )? {
                Some(name) => name,
                None => {
                    io::print_info("Operation cancelled.");
                    return Ok(());
                }
            };
            context.simulation_modify_transaction(&name)
        }
        _ => Err(CommandError::InvalidArguments(format!(
            "unknown simulation subcommand `{}`",
            sub
        ))),
    }
}
