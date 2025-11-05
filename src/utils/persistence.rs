use chrono::{DateTime, NaiveDateTime, Utc};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::HashSet,
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{errors::LedgerError, ledger::Ledger};

const DEFAULT_DIR_NAME: &str = ".budget_core";
const LEDGER_EXTENSION: &str = "json";
const BACKUP_SUFFIX: &str = ".json.bak";
const BACKUP_DIR: &str = "backups";
const STATE_FILE: &str = "state.json";
const TMP_SUFFIX: &str = "tmp";
const DEFAULT_RETENTION: usize = 5;

/// Writes the provided ledger to disk atomically.
pub fn save_ledger_to_file(ledger: &Ledger, path: &Path) -> Result<(), LedgerError> {
    let json = serde_json::to_string_pretty(ledger)?;
    fs::write(path, json)?;
    Ok(())
}

/// Loads a ledger snapshot directly from disk.
pub fn load_ledger_from_file(path: &Path) -> Result<Ledger, LedgerError> {
    let data = fs::read_to_string(path)?;
    let mut ledger: Ledger = serde_json::from_str(&data)?;
    let original_version = ledger.schema_version;
    ledger.migrate_from_schema(original_version);
    ledger.refresh_recurrence_metadata();
    Ok(ledger)
}

/// Metadata describing a restored ledger load.
#[derive(Debug)]
pub struct LoadReport {
    pub ledger: Ledger,
    pub warnings: Vec<String>,
    pub migrations: Vec<String>,
    pub path: PathBuf,
    pub name: Option<String>,
}

/// Backup metadata used when listing and restoring snapshots.
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub timestamp: DateTime<Utc>,
}

/// Metadata describing saved configuration backups.
#[derive(Debug, Clone)]
pub struct ConfigBackupInfo {
    pub path: PathBuf,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Centralized persistence layer responsible for locating, saving, and backing up ledgers.
#[derive(Debug, Clone)]
pub struct LedgerStore {
    base_dir: PathBuf,
    backup_retention: usize,
}

impl LedgerStore {
    /// Constructs a store rooted at the provided directory, falling back to defaults when omitted.
    pub fn new(
        base_dir: Option<PathBuf>,
        backup_retention: Option<usize>,
    ) -> Result<Self, LedgerError> {
        let dir = base_dir.unwrap_or_else(default_base_dir);
        fs::create_dir_all(&dir)?;
        let retention = backup_retention.unwrap_or(DEFAULT_RETENTION);
        Ok(Self {
            base_dir: dir,
            backup_retention: retention.max(1),
        })
    }

    /// Returns a store using the default base directory.
    pub fn default() -> Result<Self, LedgerError> {
        Self::new(None, None)
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn ledger_path(&self, name: &str) -> PathBuf {
        self.ledger_path_for(name)
    }

    fn state_path(&self) -> PathBuf {
        self.base_dir.join(STATE_FILE)
    }

    fn ledger_path_for(&self, name: &str) -> PathBuf {
        let file_name = format!("{}.{}", canonical_name(name), LEDGER_EXTENSION);
        self.base_dir.join(file_name)
    }

    fn backups_dir_for(&self, name: &str) -> PathBuf {
        self.base_dir.join(BACKUP_DIR).join(canonical_name(name))
    }

    /// Loads a ledger by its friendly name (mapped to a JSON file under the store).
    pub fn load_named(&self, name: &str) -> Result<LoadReport, LedgerError> {
        let path = self.ledger_path_for(name);
        self.load_from_path(&path).map(|mut report| {
            report.name = Some(name.to_string());
            report
        })
    }

    /// Loads a ledger from an arbitrary path. Warnings are returned alongside the ledger.
    pub fn load_from_path(&self, path: &Path) -> Result<LoadReport, LedgerError> {
        let data = fs::read_to_string(path)?;
        let mut ledger: Ledger = serde_json::from_str(&data)?;
        let original_version = ledger.schema_version;
        let migrations = ledger.migrate_from_schema(original_version);
        ledger.refresh_recurrence_metadata();
        let warnings = validate_ledger(&ledger);
        Ok(LoadReport {
            name: None,
            ledger,
            migrations,
            warnings,
            path: path.to_path_buf(),
        })
    }

    /// Saves the provided ledger under the supplied name, creating a backup beforehand.
    pub fn save_named(&self, ledger: &mut Ledger, name: &str) -> Result<PathBuf, LedgerError> {
        let path = self.ledger_path_for(name);
        self.save_to_path(ledger, &path)?;
        self.record_last_ledger(Some(name))?;
        Ok(path)
    }

    /// Saves to an arbitrary path, performing atomic write semantics.
    pub fn save_to_path(&self, ledger: &mut Ledger, path: &Path) -> Result<(), LedgerError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(ledger)?;
        validate_json(&json)?;
        if path.exists() {
            self.create_backup(path)?;
        }
        let tmp_path = tmp_path(path);
        write_atomic(&tmp_path, &json)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Creates a manual snapshot for the named ledger.
    pub fn backup_named(&self, name: &str) -> Result<PathBuf, LedgerError> {
        let path = self.ledger_path_for(name);
        if !path.exists() {
            return Err(LedgerError::Persistence(format!(
                "ledger `{}` has not been saved yet",
                name
            )));
        }
        self.create_backup(&path)
    }

    /// Lists existing backups for the provided ledger name.
    pub fn list_backups(&self, name: &str) -> Result<Vec<BackupInfo>, LedgerError> {
        let dir = self.backups_dir_for(name);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with(BACKUP_SUFFIX))
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(ts) = parse_backup_timestamp(&path) {
                entries.push(BackupInfo {
                    path,
                    timestamp: ts,
                });
            }
        }
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(entries)
    }

    /// Lists configuration backups stored under the persistence root.
    pub fn list_config_backups(&self) -> Result<Vec<ConfigBackupInfo>, LedgerError> {
        let dir = self.base_dir.join("state-backups");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let timestamp = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .map(DateTime::<Utc>::from);
            entries.push(ConfigBackupInfo { path, timestamp });
        }
        entries.sort_by(|a, b| match (&a.timestamp, &b.timestamp) {
            (Some(lhs), Some(rhs)) => rhs.cmp(lhs),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        });
        Ok(entries)
    }

    /// Restores the application configuration from a saved backup file.
    pub fn restore_config_backup(&self, backup_path: &Path) -> Result<(), LedgerError> {
        if !backup_path.exists() {
            return Err(LedgerError::Persistence(format!(
                "configuration backup `{}` not found",
                backup_path.display()
            )));
        }
        fs::copy(backup_path, self.state_path())?;
        Ok(())
    }

    /// Restores a ledger from the provided backup file.
    pub fn restore_backup(&self, name: &str, backup_path: &Path) -> Result<PathBuf, LedgerError> {
        let target = self.ledger_path_for(name);
        if !backup_path.exists() {
            return Err(LedgerError::Persistence(format!(
                "backup `{}` not found",
                backup_path.display()
            )));
        }
        // Overwrite the current ledger with the snapshot.
        fs::copy(backup_path, &target)?;
        Ok(target)
    }

    /// Records the last opened ledger in the state file.
    pub fn record_last_ledger(&self, name: Option<&str>) -> Result<(), LedgerError> {
        let mut state = self.read_state()?;
        state.last_ledger = name.map(|n| canonical_name(n));
        let data = serde_json::to_string_pretty(&state)?;
        write_atomic(&self.state_path(), &data)?;
        Ok(())
    }

    /// Returns the last ledger name saved in the store.
    pub fn last_ledger(&self) -> Result<Option<String>, LedgerError> {
        let state = self.read_state()?;
        Ok(state.last_ledger)
    }

    fn read_state(&self) -> Result<StoreState, LedgerError> {
        let path = self.state_path();
        if path.exists() {
            let data = fs::read_to_string(path)?;
            return Ok(serde_json::from_str(&data)?);
        }
        Ok(StoreState::default())
    }

    fn create_backup(&self, path: &Path) -> Result<PathBuf, LedgerError> {
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            return Err(LedgerError::Persistence(format!(
                "unable to derive ledger name from {}",
                path.display()
            )));
        };
        let slug = name.to_string();
        let backup_dir = self.backups_dir_for(&slug);
        fs::create_dir_all(&backup_dir)?;
        let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S");
        let backup_path = backup_dir.join(format!("{}{}", timestamp, BACKUP_SUFFIX));
        fs::copy(path, &backup_path)?;
        self.prune_backups(&slug)?;
        Ok(backup_path)
    }

    fn prune_backups(&self, name: &str) -> Result<(), LedgerError> {
        let backups = self.list_backups(name)?;
        if backups.len() <= self.backup_retention {
            return Ok(());
        }
        for info in backups.iter().skip(self.backup_retention) {
            let _ = fs::remove_file(&info.path);
        }
        Ok(())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreState {
    last_ledger: Option<String>,
}

fn default_base_dir() -> PathBuf {
    if let Some(custom) = env::var_os("BUDGET_CORE_HOME") {
        return PathBuf::from(custom);
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_DIR_NAME)
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let ext = match path.extension().and_then(|ext| ext.to_str()) {
        Some(existing) => format!("{}.{}", existing, TMP_SUFFIX),
        None => TMP_SUFFIX.to_string(),
    };
    tmp.set_extension(ext);
    tmp
}

fn write_atomic(path: &Path, data: &str) -> Result<(), LedgerError> {
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn canonical_name(name: &str) -> String {
    let sanitized: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            _ => '_',
        })
        .collect();
    if sanitized.trim_matches('_').is_empty() {
        "ledger".into()
    } else {
        sanitized
    }
}

fn validate_json(data: &str) -> Result<(), LedgerError> {
    serde_json::from_str::<serde_json::Value>(data)?;
    Ok(())
}

fn validate_ledger(ledger: &Ledger) -> Vec<String> {
    let account_ids: HashSet<_> = ledger.accounts.iter().map(|a| a.id).collect();
    let category_ids: HashSet<_> = ledger.categories.iter().map(|c| c.id).collect();
    let mut warnings = Vec::new();

    for txn in &ledger.transactions {
        if !account_ids.contains(&txn.from_account) {
            warnings.push(format!(
                "transaction {} references unknown from_account {}",
                txn.id, txn.from_account
            ));
        }
        if !account_ids.contains(&txn.to_account) {
            warnings.push(format!(
                "transaction {} references unknown to_account {}",
                txn.id, txn.to_account
            ));
        }
        if let Some(category) = txn.category_id {
            if !category_ids.contains(&category) {
                warnings.push(format!(
                    "transaction {} references missing category {}",
                    txn.id, category
                ));
            }
        }
        if let Some(rule) = txn.recurrence.as_ref() {
            if !rule.is_active() && rule.next_scheduled.is_none() {
                warnings.push(format!(
                    "recurrence {} inactive with no next date",
                    rule.series_id
                ));
            }
        }
    }

    warnings
}

fn parse_backup_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    let stem = path.file_stem()?.to_str()?;
    let ts = stem.trim_end_matches(".json");
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H-%M-%S")
        .ok()
        .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
}
