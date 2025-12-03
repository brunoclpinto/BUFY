use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crate::cli::ui::test_mode;
use crate::ledger::recurring::snapshot_recurrences;
use crate::ledger::RecurrenceSnapshot;
use crate::ledger::{Account, Ledger, Transaction};
use bufy_domain::transaction::Recurrence;

pub fn run_list_recurring(context: &mut ShellContext) -> CommandResult {
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
            cli_io::print_warning("No recurring schedules defined.");
            return Ok(());
        }

        let table = build_table(&entries);
        match select_row(context, &table, entries.len()) {
            RowSelection::Exit => return Ok(()),
            RowSelection::Index(index) => {
                let entry = &entries[index];
                let _ = cli_io::println_text("");
                let detail = build_detail_view(entry).render();
                let _ = cli_io::println_text(&detail);
                handle_actions(context, entry)?;
                let _ = cli_io::println_text("");
            }
        }
    }
}

#[derive(Clone)]
struct RecurringEntry {
    index: usize,
    summary: String,
    frequency: String,
    next_due: Option<NaiveDate>,
    start_date: NaiveDate,
    status: String,
    overdue: usize,
    pending: usize,
    from_account: String,
    to_account: String,
    category: String,
    amount: f64,
    recurrence: Recurrence,
}

fn gather_entries(context: &ShellContext) -> Result<Vec<RecurringEntry>, CommandError> {
    context.with_ledger(|ledger| {
        let today = Utc::now().date_naive();
        let snapshots = snapshot_map(ledger, today);
        let account_names = account_map(&ledger.accounts);
        let category_names = category_map(ledger);

        let mut entries = Vec::new();
        for (index, txn) in ledger.transactions.iter().enumerate() {
            let recurrence = match txn.recurrence.clone() {
                Some(value) => value,
                None => continue,
            };
            let series_id = txn.recurrence_series().unwrap_or(txn.id);
            let snapshot = match snapshots.get(&series_id) {
                Some(value) => value,
                None => continue,
            };
            entries.push(RecurringEntry {
                index,
                summary: format_summary(&account_names, &txn, &category_names),
                frequency: snapshot.interval_label.clone(),
                next_due: snapshot.next_due,
                start_date: recurrence.start_date,
                status: snapshot.status.to_string(),
                overdue: snapshot.overdue,
                pending: snapshot.pending,
                from_account: account_names
                    .get(&txn.from_account)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".into()),
                to_account: account_names
                    .get(&txn.to_account)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".into()),
                category: txn
                    .category_id
                    .and_then(|id| category_names.get(&id))
                    .cloned()
                    .unwrap_or_else(|| "—".into()),
                amount: txn.budgeted_amount,
                recurrence,
            });
        }

        if entries.is_empty() {
            return Ok(Vec::new());
        }
        entries.sort_by(|a, b| a.next_due.cmp(&b.next_due));
        Ok(entries)
    })
}

fn snapshot_map(ledger: &Ledger, reference: NaiveDate) -> HashMap<Uuid, RecurrenceSnapshot> {
    snapshot_recurrences(&ledger.transactions, reference)
        .into_iter()
        .map(|snap| (snap.series_id, snap))
        .collect()
}

fn account_map(accounts: &[Account]) -> HashMap<Uuid, String> {
    accounts
        .iter()
        .map(|account| (account.id, account.name.clone()))
        .collect()
}

fn category_map(ledger: &Ledger) -> HashMap<Uuid, String> {
    ledger
        .categories
        .iter()
        .map(|category| (category.id, category.name.clone()))
        .collect()
}

fn format_summary(
    account_names: &HashMap<Uuid, String>,
    txn: &Transaction,
    category_names: &HashMap<Uuid, String>,
) -> String {
    let from = account_names
        .get(&txn.from_account)
        .cloned()
        .unwrap_or_else(|| "Unknown".into());
    let to = account_names
        .get(&txn.to_account)
        .cloned()
        .unwrap_or_else(|| "Unknown".into());
    let category = txn
        .category_id
        .and_then(|id| category_names.get(&id))
        .cloned()
        .unwrap_or_else(|| "—".into());
    format!("{} → {} ({})", from, to, category)
}

fn build_table(entries: &[RecurringEntry]) -> Table {
    let rows = entries
        .iter()
        .map(|entry| {
            vec![
                entry.summary.clone(),
                entry.frequency.clone(),
                entry
                    .next_due
                    .map(|date| date.to_string())
                    .unwrap_or_else(|| "—".into()),
            ]
        })
        .collect();

    Table {
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 16,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "FREQUENCY".into(),
                min_width: 20,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "NEXT DUE".into(),
                min_width: 14,
                max_width: None,
                alignment: Alignment::Left,
            },
        ],
        rows,
        show_headers: true,
        padding: 1,
    }
}

fn build_detail_view(entry: &RecurringEntry) -> DetailView {
    DetailView::new(format!("Recurring: {}", entry.summary))
        .with_field("frequency", entry.frequency.clone())
        .with_field(
            "next_occurrence",
            entry
                .next_due
                .map(|date| date.to_string())
                .unwrap_or_else(|| "—".into()),
        )
        .with_field("status", entry.status.clone())
        .with_field("start_date", entry.start_date.to_string())
        .with_field("overdue", entry.overdue.to_string())
        .with_field("pending", entry.pending.to_string())
        .with_field("from_account", entry.from_account.clone())
        .with_field("to_account", entry.to_account.clone())
        .with_field("category", entry.category.clone())
        .with_field("amount_budgeted", format!("{:.2}", entry.amount))
}

enum RowSelection {
    Index(usize),
    Exit,
}

fn select_row(_context: &ShellContext, table: &Table, _len: usize) -> RowSelection {
    if let Some(keys) = test_mode::next_selector_events("recurring_selector") {
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

fn handle_actions(context: &mut ShellContext, entry: &RecurringEntry) -> CommandResult {
    let actions = vec![
        DetailAction::new("edit", "EDIT", "Edit this schedule"),
        DetailAction::new("delete", "DELETE", "Delete this schedule"),
        DetailAction::new("preview", "PREVIEW", "Show upcoming occurrences"),
    ];

    let action = match choose_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    if let Some(action) = action {
        match action.id.as_str() {
            "edit" => edit_schedule(context, entry)?,
            "delete" => delete_schedule(context, entry)?,
            "preview" => preview_schedule(entry)?,
            _ => {}
        }
    }
    Ok(())
}

fn choose_action(_context: &ShellContext, actions: &[DetailAction]) -> DetailActionResult {
    if let Some(keys) = test_mode::next_action_events("recurring_actions") {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }
    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}

fn edit_schedule(context: &mut ShellContext, entry: &RecurringEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Editing schedules is only available in interactive mode.");
        return Ok(());
    }
    context.recurrence_edit(entry.index)
}

fn delete_schedule(context: &mut ShellContext, entry: &RecurringEntry) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Delete recurring schedule `{}`?", entry.summary);
        let confirm = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirm {
            cli_io::print_info("Delete cancelled.");
            return Ok(());
        }
    }
    context.recurrence_clear(entry.index)
}

fn preview_schedule(entry: &RecurringEntry) -> CommandResult {
    let preview_dates = build_preview_dates(&entry.recurrence, 12);
    let mut view = DetailView::new(format!("Preview: {}", entry.summary)).with_field(
        "next_occurrence",
        entry
            .next_due
            .map(|date| date.to_string())
            .unwrap_or_else(|| "—".into()),
    );
    if preview_dates.is_empty() {
        view = view.with_field("upcoming", "No future occurrences");
    } else {
        let joined = preview_dates
            .iter()
            .map(|date| date.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        view = view.with_field("upcoming", joined);
    }
    let view_text = view.render();
    let _ = cli_io::println_text(&view_text);
    Ok(())
}

fn build_preview_dates(recurrence: &Recurrence, limit: usize) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut current = recurrence.start_date;
    let today = Utc::now().date_naive();
    let max_iterations = 500;
    let mut iterations = 0;
    while dates.len() < limit && iterations < max_iterations {
        if current >= today {
            dates.push(current);
        }
        current = recurrence.interval.next_date(current);
        iterations += 1;
    }
    dates
}
