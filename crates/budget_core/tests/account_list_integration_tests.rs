use std::sync::{Arc, RwLock};

use budget_core::cli::commands::account::list_accounts;
use budget_core::cli::core::{CliMode, ShellContext};
use budget_core::cli::registry::CommandRegistry;
use budget_core::cli::ui::test_mode::{
    install_action_events, install_selector_events, reset_action_events, reset_selector_events,
};
use budget_core::config::{Config, ConfigManager};
use budget_core::core::ledger_manager::LedgerManager;
use budget_core::ledger::{BudgetPeriod, Ledger};
use budget_core::storage::json_backend::JsonStorage;
use bufy_domain::account::{Account, AccountKind};
use crossterm::event::KeyCode;
use dialoguer::theme::ColorfulTheme;
use once_cell::sync::Lazy;
use std::sync::{Mutex, MutexGuard};
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
        current_simulation: None,
        last_command: None,
        running: true,
    }
}

fn sample_ledger() -> Ledger {
    let mut ledger = Ledger::new("Demo", BudgetPeriod::monthly());
    let checking = Account::new("Checking", AccountKind::Bank);
    let savings = Account::new("Savings", AccountKind::Savings);
    ledger.add_account(checking);
    ledger.add_account(savings);
    ledger
}

fn set_loaded_ledger(context: &mut ShellContext, ledger: Ledger) {
    let mut manager = context.ledger_manager.write().unwrap();
    manager.set_current(ledger, None, Some("Demo".into()));
}

#[test]
fn delete_action_removes_account() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    let ledger = sample_ledger();
    set_loaded_ledger(&mut context, ledger);

    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter], vec![KeyCode::Esc]],
        vec![vec![KeyCode::Down, KeyCode::Enter]],
    );
    list_accounts::run_list_accounts(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert_eq!(ledger.accounts.len(), 1);
}

#[test]
fn escape_exits_without_changes() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    let ledger = sample_ledger();
    set_loaded_ledger(&mut context, ledger);

    let _script = TestModeScript::new(vec![vec![KeyCode::Esc]], Vec::new());
    list_accounts::run_list_accounts(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert_eq!(ledger.accounts.len(), 2);
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
