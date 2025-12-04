use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::DetailAction;
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::run_selectable_table;
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use bufy_storage_json::LedgerMetadata;

pub fn run_list_ledgers(context: &mut ShellContext) -> CommandResult {
    run_selectable_table(
        context,
        "ledger_selector",
        "ledger_actions",
        Some("No ledgers found."),
        |ctx| ctx.list_ledger_metadata(),
        build_table,
        build_detail_view,
        |_| build_actions(),
        |ctx, entry, action| execute_action(ctx, entry, action.id.as_str()),
    )
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
