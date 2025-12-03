use std::sync::Mutex;

use budget_core::{
    config::ConfigManager, core::ledger_manager::LedgerManager, storage::json_backend::JsonStorage,
};
use once_cell::sync::Lazy;
use tempfile::TempDir;

/// Holds TempDir guards so temporary folders live for the duration of the test run.
static TEST_DIRS: Lazy<Mutex<Vec<TempDir>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Creates isolated managers backed by unique directories for each test.
pub fn setup_test_env() -> (LedgerManager, ConfigManager) {
    let temp = TempDir::new().expect("create temp dir");
    let base = temp.path().to_path_buf();
    TEST_DIRS.lock().expect("lock temp dir registry").push(temp);

    let storage =
        JsonStorage::new(Some(base.join("ledgers")), Some(3)).expect("create json storage backend");
    let ledger_manager = LedgerManager::new(Box::new(storage));
    let config_manager =
        ConfigManager::with_base_dir(base).expect("create config manager for temp dir");

    (ledger_manager, config_manager)
}
