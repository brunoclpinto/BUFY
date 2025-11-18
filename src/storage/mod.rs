pub mod json_backend;

use std::path::Path;

use crate::{core::errors::BudgetError, ledger::Ledger};

pub type Result<T> = std::result::Result<T, BudgetError>;

/// Abstraction over persistence backends capable of storing ledgers and snapshots.
pub trait StorageBackend: Send + Sync {
    fn save(&self, ledger: &Ledger, name: &str) -> Result<()>;
    fn load(&self, name: &str) -> Result<Ledger>;
    fn list_backups(&self, name: &str) -> Result<Vec<String>>;
    fn backup(&self, ledger: &Ledger, name: &str, note: Option<&str>) -> Result<()>;
    fn restore(&self, name: &str, backup_name: &str) -> Result<Ledger>;

    /// Optional helpers for ad-hoc file operations. Default implementations forward to
    /// managed storage when not overridden.
    fn save_to_path(&self, ledger: &Ledger, path: &Path) -> Result<()> {
        json_backend::save_ledger_to_path(ledger, path)
    }

    fn load_from_path(&self, path: &Path) -> Result<Ledger> {
        json_backend::load_ledger_from_path(path)
    }
}

pub use json_backend::{
    ConfigBackupInfo, ConfigData, ConfigSnapshot, JsonStorage, LedgerMetadata,
    CONFIG_BACKUP_SCHEMA_VERSION,
};
