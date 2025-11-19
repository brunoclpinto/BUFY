use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crate::cli::ui::test_mode;
use crate::storage::LedgerMetadata;

enum RowSelection {
    Index(usize),
    Exit,
}

pub fn run_list_ledgers(context: &mut ShellContext) -> CommandResult {
    loop {
        let metadata = context.list_ledger_metadata()?;
        if metadata.is_empty() {
            cli_io::print_warning("No ledgers found.");
            return Ok(());
        }

        let table = build_table(&metadata);
        match select_row(context, &table, metadata.len()) {
            RowSelection::Exit => return Ok(()),
            RowSelection::Index(index) => {
                let entry = &metadata[index];
                println!();
                println!("{}", build_detail_view(entry).render());
                handle_actions(context, entry)?;
                println!();
            }
        }
    }
}

fn build_table(metadata: &[LedgerMetadata]) -> Table {
    let columns = vec![
        TableColumn {
            header: "NAME".into(),
            min_width: 6,
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
            header: "BALANCE".into(),
            min_width: 16,
            max_width: None,
            alignment: Alignment::Right,
        },
    ];

    let rows = metadata
        .iter()
        .map(|entry| {
            vec![
                entry.name.clone(),
                entry.updated_at.to_rfc3339(),
                format!("{:.2} / {:.2}", entry.total_budgeted, entry.total_available),
            ]
        })
        .collect();

    Table {
        columns,
        rows,
        show_headers: true,
        padding: 1,
    }
}

fn build_detail_view(entry: &LedgerMetadata) -> DetailView {
    DetailView::new(format!("Ledger: {}", entry.name))
        .with_field("name", format!("\"{}\"", entry.name))
        .with_field("created_at", entry.created_at.to_rfc3339())
        .with_field("last_modified", entry.updated_at.to_rfc3339())
        .with_field("budget_period", entry.budget_period.0.label())
        .with_field(
            "current_period_budgeted_total",
            format!("{:.2}", entry.total_budgeted),
        )
        .with_field(
            "current_period_available_total",
            format!("{:.2}", entry.total_available),
        )
        .with_field("account_count", entry.account_count.to_string())
        .with_field("category_count", entry.category_count.to_string())
        .with_field("transaction_count", entry.transaction_count.to_string())
        .with_field("simulation_count", entry.simulation_count.to_string())
}

fn build_actions() -> Vec<DetailAction> {
    vec![
        DetailAction::new("edit", "EDIT", "Edit this ledger"),
        DetailAction::new("delete", "DELETE", "Delete this ledger"),
    ]
}

fn select_row(_context: &ShellContext, table: &Table, _len: usize) -> RowSelection {
    if let Some(keys) = test_mode::next_selector_events("ledger_selector") {
        return match ListSelector::new(table).run_simulated(&keys) {
            ListSelectionResult::Selected(index) => RowSelection::Index(index),
            ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
        };
    }

    match ListSelector::new(table).run() {
        ListSelectionResult::Selected(index) => RowSelection::Index(index),
        ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
    }
}

fn handle_actions(context: &mut ShellContext, meta: &LedgerMetadata) -> Result<(), CommandError> {
    let actions = build_actions();
    let action = match choose_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    if let Some(action) = action {
        execute_action(context, meta, action.id.as_str())?;
    }
    Ok(())
}

fn choose_action(_context: &ShellContext, actions: &[DetailAction]) -> DetailActionResult {
    if let Some(keys) = test_mode::next_action_events("ledger_actions") {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }

    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}

fn execute_action(
    context: &mut ShellContext,
    meta: &LedgerMetadata,
    action: &str,
) -> CommandResult {
    match action {
        "edit" => context.edit_ledger(meta),
        "delete" => confirm_delete(context, meta),
        _ => Ok(()),
    }
}

fn confirm_delete(context: &mut ShellContext, meta: &LedgerMetadata) -> CommandResult {
    if context.mode == CliMode::Script {
        return context.delete_ledger(meta);
    }
    let prompt = format!("Delete ledger \"{}\"?", meta.name);
    let confirmed = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
    if confirmed {
        context.delete_ledger(meta)
    } else {
        cli_io::print_info("Delete cancelled.");
        Ok(())
    }
}
