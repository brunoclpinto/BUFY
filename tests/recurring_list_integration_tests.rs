use std::sync::{Arc, RwLock};

use budget_core::cli::commands::recurring::list_recurring;
use budget_core::cli::core::{CliMode, ShellContext};
use budget_core::cli::registry::CommandRegistry;
use budget_core::cli::shell_context::SelectionOverride;
use budget_core::config::{Config, ConfigManager};
use budget_core::core::ledger_manager::LedgerManager;
use budget_core::domain::{
    account::{Account, AccountKind},
    category::{Category, CategoryKind},
    transaction::Transaction,
};
use budget_core::ledger::{BudgetPeriod, Ledger};
use budget_core::storage::json_backend::JsonStorage;
use chrono::NaiveDate;
use dialoguer::theme::ColorfulTheme;
use tempfile::TempDir;

fn build_context(temp: &TempDir) -> ShellContext {
    let storage = JsonStorage::new(Some(temp.path().to_path_buf()), Some(3)).unwrap();
    let manager = Arc::new(RwLock::new(LedgerManager::new(Box::new(storage.clone()))));
    let config_manager = Arc::new(RwLock::new(
        ConfigManager::with_base_dir(temp.path().to_path_buf()).unwrap(),
    ));
    let config = Arc::new(RwLock::new(Config::default()));
    ShellContext {
        mode: CliMode::Script,
        registry: CommandRegistry::new(),
        ledger_manager: manager,
        theme: ColorfulTheme::default(),
        storage,
        config_manager,
        config,
        ledger_path: None,
        active_simulation_name: None,
        selection_override: Some(SelectionOverride::default()),
        current_simulation: None,
        last_command: None,
        running: true,
    }
}

fn push_choices(context: &ShellContext, choices: &[Option<usize>]) {
    if let Some(overrides) = &context.selection_override {
        for choice in choices {
            overrides.push(*choice);
        }
    }
}

fn ledger_with_recurrence() -> Ledger {
    let mut ledger = Ledger::new("Demo", BudgetPeriod::monthly());
    let account_a = Account::new("Checking", AccountKind::Bank);
    let account_b = Account::new("Savings", AccountKind::Savings);
    let category = Category::new("Bills", CategoryKind::Expense);
    let from_id = account_a.id;
    let to_id = account_b.id;
    let category_id = category.id;
    ledger.add_account(account_a);
    ledger.add_account(account_b);
    ledger.add_category(category);

    let mut txn = Transaction::new(
        from_id,
        to_id,
        Some(category_id),
        NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
        42.0,
    );
    txn.set_recurrence(Some(budget_core::domain::transaction::Recurrence::new(
        NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
        budget_core::domain::common::TimeInterval {
            every: 1,
            unit: budget_core::domain::common::TimeUnit::Month,
        },
        budget_core::domain::transaction::RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(txn);
    ledger
}

fn set_loaded_ledger(context: &mut ShellContext, ledger: Ledger) {
    let mut manager = context.ledger_manager.write().unwrap();
    manager.set_current(ledger, None, Some("Demo".into()));
}

#[test]
fn delete_action_clears_recurrence() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, ledger_with_recurrence());

    push_choices(&context, &[Some(0), Some(1), None]);
    list_recurring::run_list_recurring(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert!(ledger.transactions[0].recurrence.is_none());
}

#[test]
fn escape_leaves_schedule_intact() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, ledger_with_recurrence());

    push_choices(&context, &[None]);
    list_recurring::run_list_recurring(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert!(ledger.transactions[0].recurrence.is_some());
}
