use std::sync::{Arc, RwLock};

use budget_core::cli::commands::transaction::list_transactions;
use budget_core::cli::core::{CliMode, ShellContext};
use budget_core::cli::formatters::CliFormatters;
use budget_core::cli::registry::CommandRegistry;
use budget_core::cli::system_clock::SystemClock;
use budget_core::cli::ui::{
    style,
    test_mode::{
        install_action_events, install_selector_events, reset_action_events, reset_selector_events,
    },
};
use budget_core::config::{Config, ConfigManager};
use budget_core::core::ledger_manager::LedgerManager;
use budget_core::ledger::{BudgetPeriod, Ledger};
use bufy_core::Clock;
use bufy_domain::{
    account::{Account, AccountKind},
    category::{Category, CategoryKind},
    transaction::{Transaction, TransactionStatus},
};
use bufy_storage_json::JsonLedgerStorage as JsonStorage;
use chrono::NaiveDate;
use crossterm::event::KeyCode;
use dialoguer::theme::ColorfulTheme;
use once_cell::sync::Lazy;
use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

fn build_context(temp: &TempDir) -> ShellContext {
    let storage =
        JsonStorage::with_retention(temp.path().join("ledgers"), temp.path().join("backups"), 3)
            .unwrap();
    let manager = Arc::new(RwLock::new(LedgerManager::new(Box::new(storage.clone()))));
    let config_manager = Arc::new(RwLock::new(
        ConfigManager::with_base_dir(temp.path().to_path_buf()).unwrap(),
    ));
    let config = Arc::new(RwLock::new(Config::default()));
    let formatters = CliFormatters::new(config.clone());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::default());
    ShellContext {
        mode: CliMode::Script,
        registry: CommandRegistry::new(),
        ledger_manager: manager,
        theme: ColorfulTheme::default(),
        storage,
        clock,
        formatters,
        config_manager,
        config,
        ledger_path: None,
        active_simulation_name: None,
        current_simulation: None,
        last_command: None,
        running: true,
        ui_style: style::style(),
    }
}

fn sample_ledger() -> Ledger {
    let mut ledger = Ledger::new("Demo", BudgetPeriod::monthly());
    let checking = Account::new("Checking", AccountKind::Bank);
    let savings = Account::new("Savings", AccountKind::Savings);
    let checking_id = checking.id;
    let savings_id = savings.id;
    ledger.add_account(checking);
    ledger.add_account(savings);

    let category = Category::new("Food", CategoryKind::Expense);
    let category_id = category.id;
    ledger.add_category(category);

    let planned = Transaction::new(
        checking_id,
        savings_id,
        Some(category_id),
        NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
        50.0,
    );
    let mut completed = Transaction::new(
        savings_id,
        checking_id,
        Some(category_id),
        NaiveDate::from_ymd_opt(2024, 5, 10).unwrap(),
        25.0,
    );
    completed.mark_completed(NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(), 25.0);

    ledger.add_transaction(planned);
    ledger.add_transaction(completed);
    ledger
}

fn set_loaded_ledger(context: &mut ShellContext, ledger: Ledger) {
    let mut manager = context.ledger_manager.write().unwrap();
    manager.set_current(ledger, None, Some("Demo".into()));
}

#[test]
fn delete_action_removes_transaction() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, sample_ledger());

    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter], vec![KeyCode::Esc]],
        vec![vec![KeyCode::Down, KeyCode::Enter]],
    );
    list_transactions::run_list_transactions(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert_eq!(ledger.transactions.len(), 1);
}

#[test]
fn complete_action_marks_transaction_completed() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, sample_ledger());

    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter], vec![KeyCode::Esc]],
        vec![vec![KeyCode::Down, KeyCode::Down, KeyCode::Enter]],
    );
    list_transactions::run_list_transactions(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert!(matches!(
        ledger.transactions.first().map(|txn| &txn.status),
        Some(TransactionStatus::Completed)
    ));
}

#[test]
fn escape_returns_without_changes() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, sample_ledger());

    let _script = TestModeScript::new(vec![vec![KeyCode::Esc]], Vec::new());
    list_transactions::run_list_transactions(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert_eq!(ledger.transactions.len(), 2);
}

static TEST_MODE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct TestModeScript {
    _guard: MutexGuard<'static, ()>,
}

impl TestModeScript {
    fn new(selectors: Vec<Vec<KeyCode>>, actions: Vec<Vec<KeyCode>>) -> Self {
        let guard = TEST_MODE_LOCK.lock().expect("test-mode lock");
        if selectors.is_empty() {
            reset_selector_events();
        } else {
            install_selector_events(selectors);
        }
        if actions.is_empty() {
            reset_action_events();
        } else {
            install_action_events(actions);
        }
        Self { _guard: guard }
    }
}

impl Drop for TestModeScript {
    fn drop(&mut self) {
        reset_selector_events();
        reset_action_events();
    }
}
