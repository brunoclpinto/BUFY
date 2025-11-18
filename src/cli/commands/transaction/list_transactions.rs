use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crate::cli::ui::test_mode;
use crate::core::services::TransactionService;
use crate::domain::transaction::TransactionStatus;

const NO_VALUE: &str = "—";

pub fn run_list_transactions(context: &mut ShellContext) -> CommandResult {
    {
        let manager = context.manager();
        if manager.current_handle().is_none() {
            cli_io::print_warning("No ledger loaded. Load or create a ledger first.");
            return Ok(());
        }
    }

    loop {
        let entries = gather_entries(context)?;
        if entries.is_empty() {
            cli_io::print_warning("No transactions recorded.");
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

struct TransactionEntry {
    index: usize,
    id: Uuid,
    summary: String,
    date_planned: String,
    actual_date: Option<String>,
    from_account: String,
    to_account: String,
    category: String,
    budgeted: f64,
    actual: Option<f64>,
    status: TransactionStatus,
    recurrence: Option<String>,
    notes: Option<String>,
}

fn gather_entries(context: &ShellContext) -> Result<Vec<TransactionEntry>, CommandError> {
    context.with_ledger(|ledger| {
        if ledger.transactions.is_empty() {
            return Ok(Vec::new());
        }
        let account_names: HashMap<Uuid, String> = ledger
            .accounts
            .iter()
            .map(|account| (account.id, account.name.clone()))
            .collect();
        let category_names: HashMap<Uuid, String> = ledger
            .categories
            .iter()
            .map(|category| (category.id, category.name.clone()))
            .collect();

        let entries = ledger
            .transactions
            .iter()
            .enumerate()
            .map(|(index, txn)| TransactionEntry {
                index,
                id: txn.id,
                summary: format!(
                    "{} -> {} on {}",
                    account_names
                        .get(&txn.from_account)
                        .cloned()
                        .unwrap_or_else(|| "Unknown".into()),
                    account_names
                        .get(&txn.to_account)
                        .cloned()
                        .unwrap_or_else(|| "Unknown".into()),
                    txn.scheduled_date
                ),
                date_planned: txn.scheduled_date.to_string(),
                actual_date: txn.actual_date.map(|date| date.to_string()),
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
                    .unwrap_or_else(|| NO_VALUE.into()),
                budgeted: txn.budgeted_amount,
                actual: txn.actual_amount,
                status: txn.status.clone(),
                recurrence: txn
                    .recurrence
                    .as_ref()
                    .map(|rule| format!("{} • {}", rule.interval.label(), rule.mode)),
                notes: txn.notes.clone(),
            })
            .collect();
        Ok(entries)
    })
}

fn build_table(entries: &[TransactionEntry]) -> Table {
    let rows = entries
        .iter()
        .map(|entry| {
            vec![
                entry.date_planned.clone(),
                entry.from_account.clone(),
                entry.to_account.clone(),
                entry.category.clone(),
                entry.status.to_string(),
                format_amount_pair(entry.budgeted, entry.actual),
            ]
        })
        .collect();

    Table {
        columns: vec![
            TableColumn {
                header: "DATE".into(),
                min_width: 10,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "FROM".into(),
                min_width: 12,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "TO".into(),
                min_width: 12,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "CATEGORY".into(),
                min_width: 12,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "STATUS".into(),
                min_width: 10,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "AMOUNT".into(),
                min_width: 18,
                max_width: None,
                alignment: Alignment::Right,
            },
        ],
        rows,
        show_headers: true,
        padding: 1,
    }
}

fn format_amount_pair(budgeted: f64, actual: Option<f64>) -> String {
    match actual {
        Some(value) => format!("{:.2} / {:.2}", budgeted, value),
        None => format!("{:.2} / {}", budgeted, NO_VALUE),
    }
}

fn build_detail_view(entry: &TransactionEntry) -> DetailView {
    let mut view = DetailView::new(format!(
        "Transaction: {} - {}",
        entry.date_planned, entry.category
    ))
    .with_field("status", entry.status.to_string())
    .with_field("from_account", entry.from_account.clone())
    .with_field("to_account", entry.to_account.clone())
    .with_field("category", entry.category.clone())
    .with_field("amount_budgeted", format!("{:.2}", entry.budgeted))
    .with_field(
        "amount_actual",
        entry
            .actual
            .map(|value| format!("{:.2}", value))
            .unwrap_or_else(|| NO_VALUE.into()),
    )
    .with_field("date_planned", entry.date_planned.clone());

    if let Some(actual) = &entry.actual_date {
        view = view.with_field("date_actual", actual.clone());
    }

    if let Some(recurrence) = &entry.recurrence {
        view = view.with_field("recurrence", recurrence.clone());
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

fn build_actions(entry: &TransactionEntry) -> Vec<DetailAction> {
    let mut actions = vec![
        DetailAction::new("edit", "EDIT", "Edit this transaction"),
        DetailAction::new("delete", "DELETE", "Delete this transaction"),
    ];
    if matches!(entry.status, TransactionStatus::Planned) {
        actions.push(DetailAction::new(
            "complete",
            "COMPLETE",
            "Mark as completed",
        ));
    }
    actions
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

    if let Some(keys) = test_mode::next_selector_events("transaction_selector") {
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

fn handle_actions(context: &mut ShellContext, entry: &TransactionEntry) -> CommandResult {
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

    if let Some(keys) = test_mode::next_action_events("transaction_actions") {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }

    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}

fn execute_action(
    context: &mut ShellContext,
    entry: &TransactionEntry,
    action: &str,
) -> CommandResult {
    match action {
        "edit" => edit_transaction(context, entry),
        "delete" => delete_transaction(context, entry),
        "complete" => complete_transaction(context, entry),
        _ => Ok(()),
    }
}

fn edit_transaction(context: &mut ShellContext, entry: &TransactionEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Transaction editing is only available in interactive mode.");
        return Ok(());
    }
    context.run_transaction_edit_wizard(entry.index)
}

fn delete_transaction(context: &mut ShellContext, entry: &TransactionEntry) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Delete transaction `{}`?", entry.summary);
        let confirmed = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirmed {
            cli_io::print_info("Delete cancelled.");
            return Ok(());
        }
    }

    context.with_ledger_mut(|ledger| {
        TransactionService::remove(ledger, entry.id).map_err(CommandError::from)
    })?;
    cli_io::print_success(format!("Transaction removed: {}", entry.summary));
    Ok(())
}

fn complete_transaction(context: &mut ShellContext, entry: &TransactionEntry) -> CommandResult {
    if !matches!(entry.status, TransactionStatus::Planned) {
        return Ok(());
    }

    if context.mode != CliMode::Interactive {
        return auto_complete_transaction(context, entry);
    }

    let actions = vec![
        DetailAction::new("auto", "AUTO COMPLETE", "Use today's date"),
        DetailAction::new("manual", "MANUAL", "Choose actual date and amount"),
        DetailAction::new("cancel", "CANCEL", "Return to list"),
    ];

    let action = match choose_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    match action.map(|a| a.id) {
        Some(id) if id == "auto" => auto_complete_transaction(context, entry),
        Some(id) if id == "manual" => manual_complete_transaction(context, entry),
        _ => Ok(()),
    }
}

fn auto_complete_transaction(
    context: &mut ShellContext,
    entry: &TransactionEntry,
) -> CommandResult {
    let today = Utc::now().date_naive();
    let amount = entry.actual.unwrap_or(entry.budgeted);
    context.with_ledger_mut(|ledger| {
        TransactionService::update(ledger, entry.id, |txn| txn.mark_completed(today, amount))
            .map_err(CommandError::from)
    })?;
    cli_io::print_success(format!("Transaction completed: {}", entry.summary));
    Ok(())
}

fn manual_complete_transaction(
    context: &mut ShellContext,
    entry: &TransactionEntry,
) -> CommandResult {
    let index_token = entry.index.to_string();
    let args = [index_token.as_str()];
    context.transaction_complete_internal(
        &args,
        "usage: transaction complete <transaction_index> <YYYY-MM-DD> <amount>",
        "Select a transaction to complete:",
    )
}
