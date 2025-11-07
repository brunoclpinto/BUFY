use std::path::{Path, PathBuf};

use crate::errors::LedgerError;
use crate::ledger::ledger::CURRENT_SCHEMA_VERSION;
use crate::ledger::Ledger;
use crate::utils::persistence::{
    BackupInfo, ConfigBackupInfo, ConfigData, ConfigSnapshot, LedgerStore, LoadReport,
};

/// Trait that abstracts interaction with the persistence layer.
pub trait StorageBackend: Send + Sync {
    fn load_named(&self, name: &str) -> Result<LoadReport, LedgerError>;
    fn load_from_path(&self, path: &Path) -> Result<LoadReport, LedgerError>;
    fn save_named(&self, ledger: &mut Ledger, name: &str) -> Result<PathBuf, LedgerError>;
    fn save_to_path(&self, ledger: &mut Ledger, path: &Path) -> Result<(), LedgerError>;
    fn ledger_path(&self, name: &str) -> PathBuf;
    fn last_ledger(&self) -> Result<Option<String>, LedgerError>;
    fn record_last_ledger(&self, name: Option<&str>) -> Result<(), LedgerError>;
    fn backup_named(&self, name: &str, note: Option<&str>) -> Result<PathBuf, LedgerError>;
    fn list_backups(&self, name: &str) -> Result<Vec<BackupInfo>, LedgerError>;
    fn restore_backup(&self, name: &str, path: &Path) -> Result<PathBuf, LedgerError>;
    fn list_config_backups(&self) -> Result<Vec<ConfigBackupInfo>, LedgerError>;
    fn create_config_backup(&self, snapshot: &ConfigSnapshot) -> Result<PathBuf, LedgerError>;
    fn load_config_snapshot(&self, path: &Path) -> Result<ConfigSnapshot, LedgerError>;
    fn save_active_config(&self, config: &ConfigData) -> Result<(), LedgerError>;
}

impl StorageBackend for LedgerStore {
    fn load_named(&self, name: &str) -> Result<LoadReport, LedgerError> {
        LedgerStore::load_named(self, name)
    }

    fn load_from_path(&self, path: &Path) -> Result<LoadReport, LedgerError> {
        LedgerStore::load_from_path(self, path)
    }

    fn save_named(&self, ledger: &mut Ledger, name: &str) -> Result<PathBuf, LedgerError> {
        LedgerStore::save_named(self, ledger, name)
    }

    fn save_to_path(&self, ledger: &mut Ledger, path: &Path) -> Result<(), LedgerError> {
        LedgerStore::save_to_path(self, ledger, path)
    }

    fn ledger_path(&self, name: &str) -> PathBuf {
        LedgerStore::ledger_path(self, name)
    }

    fn last_ledger(&self) -> Result<Option<String>, LedgerError> {
        LedgerStore::last_ledger(self)
    }

    fn record_last_ledger(&self, name: Option<&str>) -> Result<(), LedgerError> {
        LedgerStore::record_last_ledger(self, name)
    }

    fn backup_named(&self, name: &str, note: Option<&str>) -> Result<PathBuf, LedgerError> {
        LedgerStore::backup_named(self, name, note)
    }

    fn list_backups(&self, name: &str) -> Result<Vec<BackupInfo>, LedgerError> {
        LedgerStore::list_backups(self, name)
    }

    fn restore_backup(&self, name: &str, path: &Path) -> Result<PathBuf, LedgerError> {
        LedgerStore::restore_backup(self, name, path)
    }

    fn list_config_backups(&self) -> Result<Vec<ConfigBackupInfo>, LedgerError> {
        LedgerStore::list_config_backups(self)
    }

    fn create_config_backup(&self, snapshot: &ConfigSnapshot) -> Result<PathBuf, LedgerError> {
        LedgerStore::create_config_backup(self, snapshot)
    }

    fn load_config_snapshot(&self, path: &Path) -> Result<ConfigSnapshot, LedgerError> {
        LedgerStore::load_config_snapshot(self, path)
    }

    fn save_active_config(&self, config: &ConfigData) -> Result<(), LedgerError> {
        LedgerStore::save_active_config(self, config)
    }
}

/// Metadata describing the outcome of a load operation.
#[derive(Debug, Clone)]
pub struct LoadMetadata {
    pub warnings: Vec<String>,
    pub migrations: Vec<String>,
    pub path: PathBuf,
    pub name: Option<String>,
    pub schema_version: u8,
}

/// Facade that coordinates ledger state, persistence, and backups.
pub struct LedgerManager {
    pub current: Option<Ledger>,
    current_name: Option<String>,
    current_path: Option<PathBuf>,
    storage: Box<dyn StorageBackend>,
}

impl LedgerManager {
    pub fn new(storage: Box<dyn StorageBackend>) -> Self {
        Self {
            current: None,
            current_name: None,
            current_path: None,
            storage,
        }
    }

    pub fn storage(&self) -> &dyn StorageBackend {
        self.storage.as_ref()
    }

    pub fn load(&mut self, name: &str) -> Result<LoadMetadata, LedgerError> {
        let report = self.storage.load_named(name)?;
        self.ensure_schema_support(report.schema_version)?;
        self.apply_load(report)
    }

    pub fn load_from_path(&mut self, path: &Path) -> Result<LoadMetadata, LedgerError> {
        let report = self.storage.load_from_path(path)?;
        self.ensure_schema_support(report.schema_version)?;
        self.apply_load(report)
    }

    pub fn save(&mut self) -> Result<PathBuf, LedgerError> {
        let mut snapshot = self
            .current
            .clone()
            .ok_or_else(|| LedgerError::Persistence("no ledger loaded".into()))?;
        if let Some(name) = self.current_name.clone() {
            let path = self.storage.save_named(&mut snapshot, &name)?;
            self.current = Some(snapshot);
            self.current_path = Some(path.clone());
            Ok(path)
        } else if let Some(path) = self.current_path.clone() {
            self.storage.save_to_path(&mut snapshot, &path)?;
            self.current = Some(snapshot);
            Ok(path)
        } else {
            Err(LedgerError::Persistence(
                "unable to determine save target for current ledger".into(),
            ))
        }
    }

    pub fn save_as(&mut self, name: &str) -> Result<PathBuf, LedgerError> {
        let mut snapshot = self
            .current
            .clone()
            .ok_or_else(|| LedgerError::Persistence("no ledger loaded".into()))?;
        let path = self.storage.save_named(&mut snapshot, name)?;
        self.current = Some(snapshot);
        self.current_name = Some(name.to_string());
        self.current_path = Some(path.clone());
        Ok(path)
    }

    pub fn save_to_path(&mut self, path: &Path) -> Result<(), LedgerError> {
        let mut snapshot = self
            .current
            .clone()
            .ok_or_else(|| LedgerError::Persistence("no ledger loaded".into()))?;
        self.storage.save_to_path(&mut snapshot, path)?;
        self.current = Some(snapshot);
        self.current_path = Some(path.to_path_buf());
        self.current_name = None;
        Ok(())
    }

    pub fn backup(&self, note: Option<&str>) -> Result<PathBuf, LedgerError> {
        let name = self
            .current_name
            .as_deref()
            .ok_or_else(|| LedgerError::Persistence("current ledger is unnamed".into()))?;
        self.storage.backup_named(name, note)
    }

    pub fn backup_named(&self, name: &str, note: Option<&str>) -> Result<PathBuf, LedgerError> {
        self.storage.backup_named(name, note)
    }

    pub fn list_backups(&self, name: &str) -> Result<Vec<BackupInfo>, LedgerError> {
        self.storage.list_backups(name)
    }

    pub fn restore_backup(&self, name: &str, backup_path: &Path) -> Result<PathBuf, LedgerError> {
        self.storage.restore_backup(name, backup_path)
    }

    pub fn ledger_path(&self, name: &str) -> PathBuf {
        self.storage.ledger_path(name)
    }

    pub fn last_opened(&self) -> Result<Option<String>, LedgerError> {
        self.storage.last_ledger()
    }

    pub fn record_last_opened(&self, name: Option<&str>) -> Result<(), LedgerError> {
        self.storage.record_last_ledger(name)
    }

    pub fn list_config_backups(&self) -> Result<Vec<ConfigBackupInfo>, LedgerError> {
        self.storage.list_config_backups()
    }

    pub fn create_config_backup(&self, snapshot: &ConfigSnapshot) -> Result<PathBuf, LedgerError> {
        self.storage.create_config_backup(snapshot)
    }

    pub fn load_config_snapshot(&self, path: &Path) -> Result<ConfigSnapshot, LedgerError> {
        self.storage.load_config_snapshot(path)
    }

    pub fn save_active_config(&self, config: &ConfigData) -> Result<(), LedgerError> {
        self.storage.save_active_config(config)
    }

    pub fn current_name(&self) -> Option<&str> {
        self.current_name.as_deref()
    }

    pub fn current_path(&self) -> Option<&Path> {
        self.current_path.as_deref()
    }

    pub fn set_current(&mut self, ledger: Ledger, path: Option<PathBuf>, name: Option<String>) {
        self.current = Some(ledger);
        self.current_path = path;
        self.current_name = name;
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.current_name = None;
        self.current_path = None;
    }

    fn ensure_schema_support(&self, schema_version: u8) -> Result<(), LedgerError> {
        if schema_version > CURRENT_SCHEMA_VERSION {
            return Err(LedgerError::Persistence(format!(
                "ledger schema v{} is newer than supported v{}",
                schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(())
    }

    fn apply_load(&mut self, report: LoadReport) -> Result<LoadMetadata, LedgerError> {
        let LoadReport {
            ledger,
            warnings,
            migrations,
            path,
            name,
            schema_version,
        } = report;
        self.current = Some(ledger);
        self.current_path = Some(path.clone());
        self.current_name = name.clone();
        Ok(LoadMetadata {
            warnings,
            migrations,
            path,
            name,
            schema_version,
        })
    }
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
        let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(3)).unwrap();
        let mut manager = LedgerManager::new(Box::new(store));

        let ledger = Ledger::new("Demo", BudgetPeriod::monthly());
        manager.set_current(ledger, None, None);
        let path = manager.save_as("demo-ledger").expect("save ledger");
        assert!(path.exists());

        manager.clear();
        let metadata = manager.load("demo-ledger").expect("load ledger");
        assert_eq!(metadata.name.as_deref(), Some("demo-ledger"));
        assert!(manager.current.is_some());
        assert!(manager.current_path().is_some());
    }

    #[test]
    fn backup_uses_timestamped_names() {
        let temp = tempdir().unwrap();
        let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(3)).unwrap();
        let mut manager = LedgerManager::new(Box::new(store));
        let ledger = Ledger::new("Household", BudgetPeriod::monthly());
        manager.set_current(ledger, None, None);
        manager.save_as("household-budget").unwrap();

        let backup = manager
            .backup_named("household-budget", Some("Quarter Close"))
            .expect("create backup");
        let file_name = backup.file_name().and_then(|name| name.to_str()).unwrap();
        assert!(file_name.starts_with("household_budget_"));
        assert!(file_name.ends_with(".json"));
        assert!(file_name.contains("quarter-close"));
    }

    #[test]
    fn rejects_future_schema_versions() {
        let temp = tempdir().unwrap();
        let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(3)).unwrap();
        let mut manager = LedgerManager::new(Box::new(store));

        let path = temp.path().join("future.json");
        let mut ledger = Ledger::new("Future", BudgetPeriod::monthly());
        ledger.schema_version = CURRENT_SCHEMA_VERSION + 5;
        fs::write(&path, serde_json::to_string(&ledger).unwrap()).unwrap();

        let err = manager
            .load_from_path(&path)
            .expect_err("load future schema should fail");
        match err {
            LedgerError::Persistence(message) => {
                assert!(message.contains("newer"), "unexpected error: {message}");
            }
            other => panic!("expected persistence error, got {other:?}"),
        }
    }
}
