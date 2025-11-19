use std::collections::HashMap;

use uuid::Uuid;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crate::cli::ui::test_mode;
use crate::core::services::{AccountService, BudgetService};
use crate::ledger::AccountKind;

pub fn run_list_accounts(context: &mut ShellContext) -> CommandResult {
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
            cli_io::print_warning("No accounts in this ledger.");
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

struct AccountEntry {
    index: usize,
    id: Uuid,
    name: String,
    kind: AccountKind,
    category: String,
    currency: Option<String>,
    opening_balance: Option<f64>,
    notes: Option<String>,
    budgeted: f64,
    actual: f64,
    transaction_count: usize,
}

fn gather_entries(context: &ShellContext) -> Result<Vec<AccountEntry>, CommandError> {
    context.with_ledger(|ledger| {
        if ledger.accounts.is_empty() {
            return Ok(Vec::new());
        }
        let summary = BudgetService::summarize_current_period(ledger);
        let totals: HashMap<Uuid, (f64, f64)> = summary
            .per_account
            .iter()
            .map(|entry| (entry.account_id, (entry.totals.budgeted, entry.totals.real)))
            .collect();

        let entries = ledger
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let (budgeted, actual) = totals.get(&account.id).copied().unwrap_or((0.0, 0.0));
                let category = account
                    .category_id
                    .and_then(|id| ledger.category(id))
                    .map(|category| category.name.clone())
                    .unwrap_or_else(|| "—".into());
                let transaction_count = ledger
                    .transactions
                    .iter()
                    .filter(|txn| txn.from_account == account.id || txn.to_account == account.id)
                    .count();

                AccountEntry {
                    index,
                    id: account.id,
                    name: account.name.clone(),
                    kind: account.kind.clone(),
                    category,
                    currency: account.currency.clone(),
                    opening_balance: account.opening_balance,
                    notes: account.notes.clone(),
                    budgeted,
                    actual,
                    transaction_count,
                }
            })
            .collect();
        Ok(entries)
    })
}

fn build_table(entries: &[AccountEntry]) -> Table {
    let rows = entries
        .iter()
        .map(|entry| {
            vec![
                entry.name.clone(),
                entry.kind.to_string(),
                entry.category.clone(),
                format!("{:.2} / {:.2}", entry.budgeted, entry.actual),
            ]
        })
        .collect();

    Table {
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 8,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "TYPE".into(),
                min_width: 14,
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
                header: "BALANCE".into(),
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

fn build_detail_view(entry: &AccountEntry) -> DetailView {
    let mut view = DetailView::new(format!("Account: {}", entry.name))
        .with_field("name", format!("\"{}\"", entry.name))
        .with_field("type", entry.kind.to_string())
        .with_field("category", entry.category.clone())
        .with_field("budgeted_total", format!("{:.2}", entry.budgeted))
        .with_field("actual_total", format!("{:.2}", entry.actual))
        .with_field(
            "currency",
            entry.currency.clone().unwrap_or_else(|| "—".into()),
        )
        .with_field("linked_transactions", entry.transaction_count.to_string());

    if let Some(balance) = entry.opening_balance {
        view = view.with_field("opening_balance", format!("{:.2}", balance));
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

fn build_actions() -> Vec<DetailAction> {
    vec![
        DetailAction::new("edit", "EDIT", "Edit this account"),
        DetailAction::new("delete", "DELETE", "Delete this account"),
    ]
}

enum RowSelection {
    Index(usize),
    Exit,
}

fn select_row(_context: &ShellContext, table: &Table, _len: usize) -> RowSelection {
    if let Some(keys) = test_mode::next_selector_events("account_selector") {
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

fn handle_actions(context: &mut ShellContext, entry: &AccountEntry) -> CommandResult {
    let actions = build_actions();
    let action = match choose_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    if let Some(action) = action {
        execute_action(context, entry, action.id.as_str())?;
    }
    Ok(())
}

fn choose_action(_context: &ShellContext, actions: &[DetailAction]) -> DetailActionResult {
    if let Some(keys) = test_mode::next_action_events("account_actions") {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }

    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}

fn execute_action(context: &mut ShellContext, entry: &AccountEntry, action: &str) -> CommandResult {
    match action {
        "edit" => edit_account(context, entry),
        "delete" => delete_account(context, entry),
        _ => Ok(()),
    }
}

fn edit_account(context: &mut ShellContext, entry: &AccountEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Account editing is only available in interactive mode.");
        return Ok(());
    }
    context.run_account_edit_wizard(entry.index)
}

fn delete_account(context: &mut ShellContext, entry: &AccountEntry) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Delete account \"{}\"?", entry.name);
        let confirmed = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirmed {
            cli_io::print_info("Delete cancelled.");
            return Ok(());
        }
    }

    context.with_ledger_mut(|ledger| {
        AccountService::remove(ledger, entry.id).map_err(CommandError::from)
    })?;
    cli_io::print_success(format!("Account `{}` deleted.", entry.name));
    Ok(())
}
