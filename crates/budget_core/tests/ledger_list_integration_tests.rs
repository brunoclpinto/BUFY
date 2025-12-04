use std::sync::{Arc, RwLock};

use budget_core::cli::commands::ledger::list_ledgers;
use budget_core::cli::core::{CliMode, ShellContext};
use budget_core::cli::formatters::CliFormatters;
use budget_core::cli::registry::CommandRegistry;
use budget_core::cli::system_clock::SystemClock;
use budget_core::cli::ui::test_mode::{
    install_action_events, install_selector_events, reset_action_events, reset_selector_events,
};
use budget_core::config::{Config, ConfigManager};
use budget_core::core::ledger_manager::LedgerManager;
use budget_core::ledger::{BudgetPeriod, Ledger, TimeUnit};
use bufy_core::storage::LedgerStorage;
use bufy_core::Clock;
use bufy_storage_json::{load_ledger_from_path, JsonLedgerStorage as JsonStorage};
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
    }
}

fn save_sample_ledger(storage: &JsonStorage, name: &str) {
    let ledger = Ledger::new(name, BudgetPeriod::monthly());
    storage.save_ledger(name, &ledger).unwrap();
}

#[test]
fn delete_action_removes_ledger_file() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    save_sample_ledger(&context.storage, "Alpha");
    let path = context.storage.ledger_path("alpha");

    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter], vec![KeyCode::Esc]],
        vec![vec![KeyCode::Down, KeyCode::Enter]],
    );
    list_ledgers::run_list_ledgers(&mut context).unwrap();

    assert!(!path.exists());
}

#[test]
fn edit_action_updates_metadata() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    save_sample_ledger(&context.storage, "Beta");
    let path = context.storage.ledger_path("beta");

    std::env::set_var("BUFY_TEST_TEXT_INPUTS", "Renamed|every 2 weeks");
    let _script = TestModeScript::new(
        vec![vec![KeyCode::Enter], vec![KeyCode::Esc]],
        vec![vec![KeyCode::Enter]],
    );
    list_ledgers::run_list_ledgers(&mut context).unwrap();
    std::env::remove_var("BUFY_TEST_TEXT_INPUTS");

    let ledger = load_ledger_from_path(&path).unwrap();
    assert_eq!(ledger.name, "Renamed");
    assert_eq!(ledger.budget_period.0.every, 2);
    assert_eq!(ledger.budget_period.0.unit, TimeUnit::Week);
}

#[test]
fn escape_selection_returns_immediately() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    save_sample_ledger(&context.storage, "Gamma");

    let _script = TestModeScript::new(vec![vec![KeyCode::Esc]], Vec::new());
    list_ledgers::run_list_ledgers(&mut context).unwrap();

    let path = context.storage.ledger_path("gamma");
    assert!(path.exists());
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
