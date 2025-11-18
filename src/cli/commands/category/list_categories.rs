use std::collections::HashMap;

use uuid::Uuid;

use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crate::cli::ui::test_mode;
use crate::core::services::{BudgetService, CategoryService};
use crate::domain::category::{CategoryBudgetDefinition, CategoryKind};

const NO_VALUE: &str = "—";

pub fn run_list_categories(context: &mut ShellContext) -> CommandResult {
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
            cli_io::print_warning("No categories in this ledger.");
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

struct CategoryEntry {
    index: usize,
    id: Uuid,
    name: String,
    kind: CategoryKind,
    budget: Option<CategoryBudgetDefinition>,
    spent: f64,
    associated_accounts: Vec<String>,
    transaction_count: usize,
    notes: Option<String>,
}

fn gather_entries(context: &ShellContext) -> Result<Vec<CategoryEntry>, CommandError> {
    context.with_ledger(|ledger| {
        if ledger.categories.is_empty() {
            return Ok(Vec::new());
        }
        let summary = BudgetService::summarize_current_period(ledger);
        let spent_map: HashMap<Uuid, f64> = summary
            .per_category
            .iter()
            .filter_map(|entry| entry.category_id.map(|id| (id, entry.totals.real)))
            .collect();

        let mut txn_counts: HashMap<Uuid, usize> = HashMap::new();
        for txn in &ledger.transactions {
            if let Some(category_id) = txn.category_id {
                *txn_counts.entry(category_id).or_insert(0) += 1;
            }
        }

        let entries = ledger
            .categories
            .iter()
            .enumerate()
            .map(|(index, category)| {
                let spent = spent_map.get(&category.id).copied().unwrap_or(0.0);
                let transaction_count = txn_counts.get(&category.id).copied().unwrap_or(0);
                let mut associated_accounts: Vec<String> = ledger
                    .accounts
                    .iter()
                    .filter(|account| account.category_id == Some(category.id))
                    .map(|account| account.name.clone())
                    .collect();
                associated_accounts.sort();

                CategoryEntry {
                    index,
                    id: category.id,
                    name: category.name.clone(),
                    kind: category.kind.clone(),
                    budget: category.budget.clone(),
                    spent,
                    associated_accounts,
                    transaction_count,
                    notes: category.notes.clone(),
                }
            })
            .collect();
        Ok(entries)
    })
}

fn build_table(entries: &[CategoryEntry]) -> Table {
    let rows = entries
        .iter()
        .map(|entry| {
            vec![
                entry.name.clone(),
                entry.kind.to_string(),
                format_budget_text(entry.budget.as_ref()),
                format!("{:.2}", entry.spent),
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
                header: "TYPE".into(),
                min_width: 14,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "BUDGET".into(),
                min_width: 16,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "SPENT".into(),
                min_width: 10,
                max_width: None,
                alignment: Alignment::Right,
            },
        ],
        rows,
        show_headers: true,
        padding: 1,
    }
}

fn format_budget_text(definition: Option<&CategoryBudgetDefinition>) -> String {
    match definition {
        Some(def) => format!("{:.2} / {}", def.amount, format_budget_period(&def.period)),
        None => NO_VALUE.into(),
    }
}

fn format_budget_period(period: &crate::domain::common::BudgetPeriod) -> String {
    use crate::domain::common::BudgetPeriod::*;
    match period {
        Daily => "Daily".into(),
        Weekly => "Weekly".into(),
        Monthly => "Monthly".into(),
        Yearly => "Yearly".into(),
        Custom(days) => format!("Every {} day{}", days, if *days == 1 { "" } else { "s" }),
    }
}

fn build_detail_view(entry: &CategoryEntry) -> DetailView {
    let associated = if entry.associated_accounts.is_empty() {
        NO_VALUE.into()
    } else {
        entry.associated_accounts.join(", ")
    };

    let mut view = DetailView::new(format!("Category: {}", entry.name))
        .with_field("name", format!("\"{}\"", entry.name))
        .with_field("type", entry.kind.to_string())
        .with_field("spent_current_period", format!("{:.2}", entry.spent))
        .with_field("transaction_count", entry.transaction_count.to_string())
        .with_field("associated_accounts", associated)
        .with_field("created_at", NO_VALUE)
        .with_field("last_modified", NO_VALUE);

    if let Some(definition) = &entry.budget {
        view = view
            .with_field("budget.amount", format!("{:.2}", definition.amount))
            .with_field("budget.period", format_budget_period(&definition.period))
            .with_field(
                "budget.start_date",
                definition
                    .reference_date
                    .map(|date| date.to_string())
                    .unwrap_or_else(|| NO_VALUE.into()),
            );
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
        DetailAction::new("edit", "EDIT", "Edit this category"),
        DetailAction::new("delete", "DELETE", "Delete this category"),
        DetailAction::new("budget", "BUDGET", "Adjust category budget"),
    ]
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

    if let Some(keys) = test_mode::next_selector_events("category_selector") {
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

fn handle_actions(context: &mut ShellContext, entry: &CategoryEntry) -> CommandResult {
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
    if let Some(keys) = test_mode::next_action_events("category_actions") {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }

    DetailActionsMenu::new("Actions", actions.to_vec()).run()
}

fn execute_action(
    context: &mut ShellContext,
    entry: &CategoryEntry,
    action: &str,
) -> CommandResult {
    match action {
        "edit" => edit_category(context, entry),
        "delete" => delete_category(context, entry),
        "budget" => manage_budget(context, entry),
        _ => Ok(()),
    }
}

fn edit_category(context: &mut ShellContext, entry: &CategoryEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Category editing is only available in interactive mode.");
        return Ok(());
    }
    context.run_category_edit_wizard(entry.index)
}

fn delete_category(context: &mut ShellContext, entry: &CategoryEntry) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Delete category \"{}\"?", entry.name);
        let confirmed = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirmed {
            cli_io::print_info("Delete cancelled.");
            return Ok(());
        }
    }

    context.with_ledger_mut(|ledger| {
        CategoryService::remove(ledger, entry.id).map_err(CommandError::from)
    })?;
    cli_io::print_success(format!("Category `{}` deleted.", entry.name));
    Ok(())
}

fn manage_budget(context: &mut ShellContext, entry: &CategoryEntry) -> CommandResult {
    if context.mode != CliMode::Interactive {
        cli_io::print_info("Category budget adjustments are only available in interactive mode.");
        return Ok(());
    }

    let actions = vec![
        DetailAction::new("set", "SET", "Assign a budget"),
        DetailAction::new("edit", "EDIT", "Edit the existing budget"),
        DetailAction::new("clear", "CLEAR", "Remove the budget"),
        DetailAction::new("view", "VIEW", "Show current budget usage"),
    ];

    let action = match choose_budget_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    if let Some(action) = action {
        let name_arg = entry.name.as_str();
        match action.id.as_str() {
            "set" | "edit" => context.category_budget_set(&[name_arg])?,
            "clear" => context.category_budget_clear(&[name_arg])?,
            "view" => context.category_budget_show(&[name_arg])?,
            _ => {}
        }
    }
    Ok(())
}

fn choose_budget_action(context: &ShellContext, actions: &[DetailAction]) -> DetailActionResult {
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
    if let Some(keys) = test_mode::next_action_events("category_budget_actions") {
        return DetailActionsMenu::new("Budget Options", actions.to_vec()).run_simulated(&keys);
    }
    DetailActionsMenu::new("Budget Options", actions.to_vec()).run()
}
