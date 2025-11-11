use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

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
    pub current: Option<Arc<RwLock<Ledger>>>,
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
        self.current = Some(Arc::new(RwLock::new(ledger)));
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
        self.current = Some(Arc::new(RwLock::new(ledger)));
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
        let name = self
            .current_name
            .as_deref()
            .ok_or_else(|| BudgetError::StorageError("unnamed ledger cannot be saved".into()))?;
        {
            let ledger = self.read()?;
            self.storage.save(&ledger, name)?;
        }
        Ok(())
    }

    pub fn save_as(&mut self, name: &str) -> Result<(), BudgetError> {
        {
            let ledger = self.read()?;
            self.storage.save(&ledger, name)?;
        }
        self.current_name = Some(name.to_string());
        Ok(())
    }

    pub fn backup(&self, note: Option<&str>) -> Result<(), BudgetError> {
        let ledger = self.read()?;
        let name = self
            .current_name
            .as_deref()
            .ok_or_else(|| BudgetError::StorageError("current ledger is unnamed".into()))?;
        self.storage.backup(&ledger, name, note)
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
        self.current = Some(Arc::new(RwLock::new(ledger)));
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
        self.current = Some(Arc::new(RwLock::new(ledger)));
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

    /// Executes a closure with an immutable reference to the current ledger.
    /// Returns [`BudgetError::LedgerNotLoaded`] when no ledger is available.
    pub fn with_current<T, F>(&self, f: F) -> Result<T, BudgetError>
    where
        F: FnOnce(&Ledger) -> T,
    {
        let ledger = self.read()?;
        Ok(f(&ledger))
    }

    /// Executes a closure with a mutable reference to the current ledger.
    /// Returns [`BudgetError::LedgerNotLoaded`] when no ledger is available.
    pub fn with_current_mut<T, F>(&self, f: F) -> Result<T, BudgetError>
    where
        F: FnOnce(&mut Ledger) -> T,
    {
        let mut ledger = self.write()?;
        Ok(f(&mut ledger))
    }

    pub fn current_handle(&self) -> Option<Arc<RwLock<Ledger>>> {
        self.current.as_ref().map(Arc::clone)
    }

    pub fn read(&self) -> Result<RwLockReadGuard<'_, Ledger>, BudgetError> {
        let handle = self.current.as_ref().ok_or(BudgetError::LedgerNotLoaded)?;
        handle
            .read()
            .map_err(|_| BudgetError::StorageError("ledger lock poisoned".into()))
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, Ledger>, BudgetError> {
        let handle = self.current.as_ref().ok_or(BudgetError::LedgerNotLoaded)?;
        handle
            .write()
            .map_err(|_| BudgetError::StorageError("ledger lock poisoned".into()))
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
        assert!(manager.current_handle().is_some());
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

    #[test]
    fn with_current_helpers_access_loaded_ledger() {
        let temp = tempdir().unwrap();
        let store = crate::storage::json_backend::JsonStorage::new(
            Some(temp.path().to_path_buf()),
            Some(3),
        )
        .unwrap();
        let mut manager = LedgerManager::new(Box::new(store));
        let ledger = Ledger::new("Helpers", BudgetPeriod::monthly());
        manager.set_current(ledger, None, Some("helpers".into()));

        let name = manager
            .with_current(|ledger| ledger.name.clone())
            .expect("ledger present");
        assert_eq!(name, "Helpers");

        manager
            .with_current_mut(|ledger| ledger.name.push_str(" Updated"))
            .expect("ledger present");
        let updated = manager
            .with_current(|ledger| ledger.name.clone())
            .expect("ledger present");
        assert_eq!(updated, "Helpers Updated");
    }
}
