use std::path::{Path, PathBuf};

use crate::core::errors::BudgetError;
use crate::ledger::ledger::CURRENT_SCHEMA_VERSION;
use crate::ledger::Ledger;
use crate::storage::json_backend::ledger_warnings;
use crate::storage::StorageBackend;

/// Metadata describing the outcome of a load operation.
#[derive(Debug, Clone)]
pub struct LoadMetadata {
    pub warnings: Vec<String>,
    pub migrations: Vec<String>,
    pub path: Option<PathBuf>,
    pub name: Option<String>,
    pub schema_version: u8,
}

/// Facade that coordinates ledger state, persistence, and backups.
pub struct LedgerManager {
    pub current: Option<Ledger>,
    current_name: Option<String>,
    storage: Box<dyn StorageBackend>,
}

impl LedgerManager {
    pub fn new(storage: Box<dyn StorageBackend>) -> Self {
        Self {
            current: None,
            current_name: None,
            storage,
        }
    }

    pub fn storage(&self) -> &dyn StorageBackend {
        self.storage.as_ref()
    }

    pub fn load(&mut self, name: &str) -> Result<LoadMetadata, BudgetError> {
        let mut ledger = self.storage.load(name)?;
        let meta = self.process_loaded_ledger(&mut ledger)?;
        self.current = Some(ledger);
        self.current_name = Some(name.to_string());
        Ok(LoadMetadata {
            warnings: meta.warnings,
            migrations: meta.migrations,
            path: None,
            name: Some(name.to_string()),
            schema_version: meta.original_version,
        })
    }

    pub fn load_from_path(&mut self, path: &Path) -> Result<LoadMetadata, BudgetError> {
        let mut ledger = self.storage.load_from_path(path)?;
        let meta = self.process_loaded_ledger(&mut ledger)?;
        self.current = Some(ledger);
        self.current_name = None;
        Ok(LoadMetadata {
            warnings: meta.warnings,
            migrations: meta.migrations,
            path: Some(path.to_path_buf()),
            name: None,
            schema_version: meta.original_version,
        })
    }

    pub fn save(&mut self) -> Result<(), BudgetError> {
        let ledger = self
            .current
            .as_ref()
            .ok_or_else(|| BudgetError::StorageError("no ledger loaded".into()))?;
        let name = self
            .current_name
            .as_deref()
            .ok_or_else(|| BudgetError::StorageError("unnamed ledger cannot be saved".into()))?;
        self.storage.save(ledger, name)
    }

    pub fn save_as(&mut self, name: &str) -> Result<(), BudgetError> {
        let ledger = self
            .current
            .as_ref()
            .ok_or_else(|| BudgetError::StorageError("no ledger loaded".into()))?;
        self.storage.save(ledger, name)?;
        self.current_name = Some(name.to_string());
        Ok(())
    }

    pub fn backup(&self, note: Option<&str>) -> Result<(), BudgetError> {
        let ledger = self
            .current
            .as_ref()
            .ok_or_else(|| BudgetError::StorageError("no ledger loaded".into()))?;
        let name = self
            .current_name
            .as_deref()
            .ok_or_else(|| BudgetError::StorageError("current ledger is unnamed".into()))?;
        self.storage.backup(ledger, name, note)
    }

    pub fn list_backups(&self, name: &str) -> Result<Vec<String>, BudgetError> {
        self.storage.list_backups(name)
    }

    pub fn restore_backup(
        &mut self,
        name: &str,
        backup_name: &str,
    ) -> Result<LoadMetadata, BudgetError> {
        let mut ledger = self.storage.restore(name, backup_name)?;
        let meta = self.process_loaded_ledger(&mut ledger)?;
        self.current = Some(ledger);
        self.current_name = Some(name.to_string());
        Ok(LoadMetadata {
            warnings: meta.warnings,
            migrations: meta.migrations,
            path: None,
            name: Some(name.to_string()),
            schema_version: meta.original_version,
        })
    }

    pub fn set_current(&mut self, ledger: Ledger, path: Option<PathBuf>, name: Option<String>) {
        let _ = path;
        self.current = Some(ledger);
        self.current_name = name;
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.current_name = None;
    }

    pub fn current_name(&self) -> Option<&str> {
        self.current_name.as_deref()
    }

    pub fn clear_name(&mut self) {
        self.current_name = None;
    }

    fn process_loaded_ledger(&self, ledger: &mut Ledger) -> Result<LoadEffects, BudgetError> {
        let original_version = ledger.schema_version;
        self.ensure_schema_support(original_version)?;
        let migrations = ledger.migrate_from_schema(original_version);
        ledger.refresh_recurrence_metadata();
        let warnings = ledger_warnings(ledger);
        Ok(LoadEffects {
            original_version,
            migrations,
            warnings,
        })
    }

    fn ensure_schema_support(&self, schema_version: u8) -> Result<(), BudgetError> {
        if schema_version > CURRENT_SCHEMA_VERSION {
            return Err(BudgetError::StorageError(format!(
                "ledger schema v{} is newer than supported v{}",
                schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(())
    }
}

struct LoadEffects {
    original_version: u8,
    migrations: Vec<String>,
    warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::BudgetPeriod;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_named_roundtrip() {
        let temp = tempdir().unwrap();
        let store = crate::storage::json_backend::JsonStorage::new(
            Some(temp.path().to_path_buf()),
            Some(3),
        )
        .unwrap();
        let mut manager = LedgerManager::new(Box::new(store));

        let ledger = Ledger::new("Demo", BudgetPeriod::monthly());
        manager.set_current(ledger, None, None);
        manager.save_as("demo-ledger").expect("save ledger");

        manager.clear();
        let metadata = manager.load("demo-ledger").expect("load ledger");
        assert_eq!(metadata.name.as_deref(), Some("demo-ledger"));
        assert!(manager.current.is_some());
    }

    #[test]
    fn backup_uses_timestamped_names() {
        let temp = tempdir().unwrap();
        let store = crate::storage::json_backend::JsonStorage::new(
            Some(temp.path().to_path_buf()),
            Some(3),
        )
        .unwrap();
        let mut manager = LedgerManager::new(Box::new(store));
        let ledger = Ledger::new("Household", BudgetPeriod::monthly());
        manager.set_current(ledger.clone(), None, Some("household-budget".into()));
        manager.save_as("household-budget").unwrap();

        manager
            .backup(Some("Quarter Close"))
            .expect("create backup");
        let backups = manager.list_backups("household-budget").unwrap();
        assert!(!backups.is_empty());
        assert!(backups[0].starts_with("household_budget_"));
    }

    #[test]
    fn rejects_future_schema_versions() {
        let temp = tempdir().unwrap();
        let store = crate::storage::json_backend::JsonStorage::new(
            Some(temp.path().to_path_buf()),
            Some(3),
        )
        .unwrap();
        let mut manager = LedgerManager::new(Box::new(store));

        let path = temp.path().join("future.json");
        let mut ledger = Ledger::new("Future", BudgetPeriod::monthly());
        ledger.schema_version = CURRENT_SCHEMA_VERSION + 5;
        fs::write(&path, serde_json::to_string(&ledger).unwrap()).unwrap();

        let err = manager
            .load_from_path(&path)
            .expect_err("load future schema should fail");
        match err {
            BudgetError::StorageError(message) => {
                assert!(message.contains("newer"), "unexpected error: {message}");
            }
            other => panic!("expected persistence error, got {other:?}"),
        }
    }
}
