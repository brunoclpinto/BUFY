use crate::cli::commands::simulation_handlers;
use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};

pub fn run_list_simulations(context: &mut ShellContext) -> CommandResult {
    {
        let manager = context.manager();
        if manager.current_handle().is_none() {
            cli_io::print_warning("No ledger loaded.");
            return Ok(());
        }
    }

    loop {
        let entries = gather_entries(context)?;
        if entries.is_empty() {
            cli_io::print_warning("No simulations defined.");
            return Ok(());
        }

        let table = build_table(&entries);
        match select_row(context, &table, entries.len()) {
            RowSelection::Exit => return Ok(()),
            RowSelection::Index(index) => {
                let entry = &entries[index];
                println!();
                println!("{}", build_detail_view(entry).render());
                handle_actions(context, entry)?;
                println!();
            }
        }
    }
}

#[derive(Clone)]
struct SimulationEntry {
    name: String,
    created_at: String,
    updated_at: String,
    changes: usize,
    status: String,
    is_active: bool,
    change_summaries: Vec<String>,
    notes: Option<String>,
}

fn gather_entries(context: &ShellContext) -> Result<Vec<SimulationEntry>, CommandError> {
    let active_name = context
        .active_simulation_name()
        .map(|name| name.to_string());
    context.with_ledger(|ledger| {
        let sims = ledger.simulations();
        let entries = sims
            .iter()
            .map(|sim| SimulationEntry {
                name: sim.name.clone(),
                created_at: sim.created_at.to_rfc3339(),
                updated_at: sim.updated_at.to_rfc3339(),
                changes: sim.changes.len(),
                status: sim.status.to_string(),
                is_active: active_name
                    .as_ref()
                    .map(|name| name.eq_ignore_ascii_case(&sim.name))
                    .unwrap_or(false),
                change_summaries: sim.changes.iter().map(|change| change.summary()).collect(),
                notes: sim.notes.clone(),
            })
            .collect();
        Ok(entries)
    })
}

fn build_table(entries: &[SimulationEntry]) -> Table {
    let rows = entries
        .iter()
        .map(|entry| {
            vec![
                entry.name.clone(),
                entry.updated_at.clone(),
                entry.changes.to_string(),
                if entry.is_active {
                    "Active".into()
                } else {
                    "Inactive".into()
                },
            ]
        })
        .collect();

    Table {
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 10,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "LAST MODIFIED".into(),
                min_width: 19,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "CHANGES".into(),
                min_width: 10,
                max_width: None,
                alignment: Alignment::Right,
            },
            TableColumn {
                header: "STATUS".into(),
                min_width: 8,
                max_width: None,
                alignment: Alignment::Left,
            },
        ],
        rows,
        show_headers: true,
        padding: 1,
    }
}

fn build_detail_view(entry: &SimulationEntry) -> DetailView {
    let mut view = DetailView::new(format!("Simulation: {}", entry.name))
        .with_field("name", format!("\"{}\"", entry.name))
        .with_field(
            "status",
            if entry.is_active {
                "Active"
            } else {
                entry.status.as_str()
            },
        )
        .with_field("pending_changes", entry.changes.to_string())
        .with_field("created_at", entry.created_at.clone())
        .with_field("last_modified", entry.updated_at.clone());

    if !entry.change_summaries.is_empty() {
        view = view.with_field("change_summary", entry.change_summaries.join(", "));
    }

    if let Some(notes) = entry
        .notes
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        view = view.with_field("notes", notes.clone());
    }

    view
}

enum RowSelection {
    Index(usize),
    Exit,
}

fn select_row(context: &ShellContext, table: &Table, len: usize) -> RowSelection {
    if let Some(choice) = context.take_override_choice() {
        return match choice {
            Some(index) if index < len => RowSelection::Index(index),
            _ => RowSelection::Exit,
        };
    }

    match ListSelector::new(table).run() {
        ListSelectionResult::Selected(index) => RowSelection::Index(index),
        ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
    }
}

fn handle_actions(context: &mut ShellContext, entry: &SimulationEntry) -> CommandResult {
    let actions = build_actions(entry);
    let action = match choose_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    if let Some(action) = action {
        execute_action(context, entry, action.id.as_str())?;
    }
    Ok(())
}

fn build_actions(entry: &SimulationEntry) -> Vec<DetailAction> {
    let mut actions = vec![
        DetailAction::new("edit", "EDIT", "Edit this simulation"),
        DetailAction::new("discard", "DISCARD", "Discard this simulation"),
        DetailAction::new("apply", "APPLY", "Apply this simulation"),
    ];
    if !entry.is_active {
        actions.push(DetailAction::new(
            "enter",
            "ENTER",
            "Enter this simulation for editing",
        ));
    }
    actions
}

fn choose_action(context: &ShellContext, actions: &[DetailAction]) -> DetailActionResult {
    if let Some(choice) = context.take_override_choice() {
        return match choice {
            Some(index) => actions
                .get(index)
                .cloned()
                .map(DetailActionResult::Selected)
                .unwrap_or(DetailActionResult::Escaped),
            None => DetailActionResult::Escaped,
        };
    }
    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}

fn execute_action(
    context: &mut ShellContext,
    entry: &SimulationEntry,
    action: &str,
) -> CommandResult {
    match action {
        "edit" => edit_simulation(context, entry),
        "discard" => discard_simulation(context, entry),
        "apply" => apply_simulation(context, entry),
        "enter" => enter_simulation(context, entry),
        _ => Ok(()),
    }
}

fn edit_simulation(context: &mut ShellContext, entry: &SimulationEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Simulation editing is only available in interactive mode.");
        return Ok(());
    }
    simulation_handlers::handle_workflow_action(context, "modify", &[entry.name.as_str()])
}

fn discard_simulation(context: &mut ShellContext, entry: &SimulationEntry) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Discard simulation \"{}\"?", entry.name);
        let confirm = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirm {
            cli_io::print_info("Discard cancelled.");
            return Ok(());
        }
    }
    simulation_handlers::handle_discard(context, &[entry.name.as_str()])
}

fn apply_simulation(context: &mut ShellContext, entry: &SimulationEntry) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Apply simulation \"{}\" to the ledger?", entry.name);
        let confirm = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirm {
            cli_io::print_info("Apply cancelled.");
            return Ok(());
        }
    }
    simulation_handlers::handle_apply(context, &[entry.name.as_str()])
}

fn enter_simulation(context: &mut ShellContext, entry: &SimulationEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Entering simulations is only available in interactive mode.");
        return Ok(());
    }
    simulation_handlers::handle_enter(context, &[entry.name.as_str()])
}
