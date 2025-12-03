use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use budget_core::cli::commands::backup::list_backups;
use budget_core::cli::core::{CliMode, ShellContext};
use budget_core::cli::registry::CommandRegistry;
use budget_core::cli::ui::test_mode::{
    install_action_events, install_selector_events, reset_action_events, reset_selector_events,
};
use budget_core::config::{Config, ConfigManager};
use budget_core::core::ledger_manager::LedgerManager;
use budget_core::ledger::{BudgetPeriod, Ledger};
use bufy_domain::account::{Account, AccountKind};
use bufy_storage_json::JsonLedgerStorage as JsonStorage;
use crossterm::event::KeyCode;
use dialoguer::theme::ColorfulTheme;
use once_cell::sync::Lazy;
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

fn set_loaded_ledger(context: &mut ShellContext, name: &str) {
    let ledger = Ledger::new(name, BudgetPeriod::monthly());
    {
        let mut manager = context.ledger_manager.write().unwrap();
        manager.set_current(ledger, None, Some(name.into()));
        manager.save_as(name).unwrap();
        manager.backup(None).unwrap();
    }
    context.ledger_path = Some(context.storage.ledger_path(name));
}

#[test]
fn delete_action_removes_backup() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, "Demo");

    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter]],
        vec![vec![KeyCode::Down, KeyCode::Enter]],
    );
    list_backups::run_list_backups(&mut context).unwrap();

    let metadata = context
        .storage
        .list_backup_metadata("Demo")
        .expect("metadata");
    assert!(metadata.is_empty());
}

#[test]
fn escape_leaves_backups_untouched() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, "Demo");

    let _script = TestModeScript::new(vec![vec![KeyCode::Esc]], Vec::new());
    list_backups::run_list_backups(&mut context).unwrap();

    let metadata = context
        .storage
        .list_backup_metadata("Demo")
        .expect("metadata");
    assert_eq!(metadata.len(), 1);
}

#[test]
fn restore_action_restores_backup_state() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, "Demo");

    // mutate ledger after backup
    {
        let manager = context.ledger_manager.read().unwrap();
        let handle = manager.current_handle().expect("ledger loaded");
        let mut ledger = handle.write().unwrap();
        ledger.add_account(Account::new("Temp", AccountKind::Bank));
    }
    {
        let manager = context.ledger_manager.read().unwrap();
        let handle = manager.current_handle().expect("ledger loaded");
        let ledger = handle.read().unwrap();
        assert_eq!(ledger.accounts.len(), 1, "mutation should be visible");
    }

    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter], vec![KeyCode::Esc]],
        vec![vec![KeyCode::Enter]],
    );
    list_backups::run_list_backups(&mut context).unwrap();

    let manager = context.ledger_manager.read().unwrap();
    let handle = manager.current_handle().expect("ledger loaded");
    let ledger = handle.read().unwrap();
    assert!(
        ledger.accounts.is_empty(),
        "restore should revert to original backup state"
    );
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
