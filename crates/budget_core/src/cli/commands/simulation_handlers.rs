use chrono::Local;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io;
use crate::ledger::{LedgerExt, SimulationStatus};

pub fn handle_create(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = if let Some(name) = args.first() {
        (*name).to_string()
    } else {
        loop {
            let value = io::prompt_text("Simulation name", None).map_err(CommandError::from)?;
            let Some(value) = value else {
                io::print_info("Operation cancelled.");
                return Ok(());
            };
            let trimmed = value.trim();
            if trimmed.is_empty() {
                io::print_error("Name cannot be empty.");
                continue;
            }
            break trimmed.to_string();
        }
    };
    let notes: Option<String> = if context.mode() == CliMode::Interactive {
        let response = io::prompt_text("Notes (optional)", None).map_err(CommandError::from)?;
        let Some(text) = response else {
            io::print_info("Operation cancelled.");
            return Ok(());
        };
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
            .map_err(CommandError::from)
    })?;
    io::print_success(format!("Simulation `{}` created.", name));
    Ok(())
}

pub fn handle_enter(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = resolve_simulation_name(
        context,
        args.first().copied(),
        "Select a simulation to enter:",
        false,
        "usage: simulation enter <name>",
    )?;
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

pub fn handle_leave(context: &mut ShellContext) -> CommandResult {
    if context.active_simulation_name().is_none() {
        return Err(CommandError::InvalidArguments(
            "No active simulation to leave".into(),
        ));
    }
    context.clear_active_simulation();
    io::print_success("Simulation mode cleared.");
    Ok(())
}

pub fn handle_apply(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = resolve_simulation_name(
        context,
        args.first().copied(),
        "Select a simulation to apply:",
        false,
        "usage: simulation apply <name>",
    )?;
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
            .map_err(CommandError::from)
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

pub fn handle_discard(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    let name = resolve_simulation_name(
        context,
        args.first().copied(),
        "Select a simulation to discard:",
        false,
        "usage: simulation discard <name>",
    )?;
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
            .map_err(CommandError::from)
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

pub fn handle_workflow_action(
    context: &mut ShellContext,
    action: &str,
    args: &[&str],
) -> CommandResult {
    let usage = match action {
        "add" => "usage: simulation add [simulation_name]",
        "exclude" => "usage: simulation exclude [simulation_name]",
        "modify" => "usage: simulation modify [simulation_name]",
        _ => "usage: simulation changes [simulation_name]",
    };
    let prompt = match action {
        "add" => "Select a simulation to add a transaction to:",
        "exclude" => "Select a simulation to exclude a transaction from:",
        "modify" => "Select a simulation to modify:",
        _ => "Select a simulation to inspect:",
    };
    let name = resolve_simulation_name(context, args.get(0).copied(), prompt, true, usage)?;
    match action {
        "changes" | "show" => context.print_simulation_changes(&name),
        "add" => context.simulation_add_transaction(&name),
        "exclude" => context.simulation_exclude_transaction(&name),
        "modify" => context.simulation_modify_transaction(&name),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown simulation subcommand `{}`",
            other
        ))),
    }
}

fn resolve_simulation_name(
    context: &mut ShellContext,
    arg: Option<&str>,
    prompt: &str,
    allow_cancel: bool,
    usage: &str,
) -> Result<String, CommandError> {
    match context.resolve_simulation_name(arg, prompt, allow_cancel, usage)? {
        Some(name) => Ok(name),
        None => {
            io::print_info("Operation cancelled.");
            Err(CommandError::Message("Operation cancelled".into()))
        }
    }
}
