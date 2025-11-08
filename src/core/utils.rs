use dirs::home_dir;
use std::{env, path::PathBuf};

const DEFAULT_DIR_NAME: &str = ".budget_core";
const LEDGER_DIR: &str = "ledgers";
const BACKUP_DIR: &str = "backups";
const CONFIG_BACKUP_DIR: &str = "config_backups";
const STATE_FILE: &str = "state.json";

/// Returns the application-specific data directory, defaulting to `~/.budget_core`.
pub fn app_data_dir() -> PathBuf {
    if let Some(custom) = env::var_os("BUDGET_CORE_HOME") {
        return PathBuf::from(custom);
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_DIR_NAME)
}

/// Absolute path to the managed ledgers directory.
pub fn ledgers_dir() -> PathBuf {
    app_data_dir().join(LEDGER_DIR)
}

/// Resolves the canonical file path for a ledger name (slug applied upstream).
pub fn ledger_file(name: &str) -> PathBuf {
    ledgers_dir().join(format!("{}.json", name))
}

/// Base directory for backup snapshots.
pub fn backups_root() -> PathBuf {
    app_data_dir().join(BACKUP_DIR)
}

/// Returns the directory containing configuration backups.
pub fn config_backups_dir() -> PathBuf {
    app_data_dir().join(CONFIG_BACKUP_DIR)
}

/// Path to the shared state file (tracking last opened ledger, etc.).
pub fn state_file() -> PathBuf {
    app_data_dir().join(STATE_FILE)
}
