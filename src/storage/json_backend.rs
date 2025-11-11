use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    core::{
        errors::BudgetError,
        utils::{ensure_dir, PathResolver},
    },
    currency::{CurrencyCode, CurrencyDisplay, LocaleConfig, NegativeStyle, ValuationPolicy},
    ledger::Ledger,
};

use super::{Result, StorageBackend};

const BACKUP_EXTENSION: &str = "json";
const BACKUP_TIMESTAMP_FORMAT: &str = "%Y%m%d_%H%M";
const TMP_SUFFIX: &str = "tmp";
const DEFAULT_RETENTION: usize = 5;

pub const CONFIG_BACKUP_SCHEMA_VERSION: u32 = 1;

#[derive(Clone)]
pub struct JsonStorage {
    root: PathBuf,
    ledgers_dir: PathBuf,
    backups_dir: PathBuf,
    config_dir: PathBuf,
    state_file: PathBuf,
    retention: usize,
}

impl JsonStorage {
    pub fn new(root: Option<PathBuf>, retention: Option<usize>) -> Result<Self> {
        let app_root = PathResolver::resolve_base(root);
        ensure_dir(&app_root)?;
        let ledgers_dir = PathResolver::ledger_dir_in(&app_root);
        let backups_dir = PathResolver::backup_dir_in(&app_root);
        let config_dir = PathResolver::config_backup_dir_in(&app_root);
        ensure_dir(&ledgers_dir)?;
        ensure_dir(&backups_dir)?;
        ensure_dir(&config_dir)?;
        let state_file = PathResolver::state_file_in(&app_root);
        Ok(Self {
            root: app_root,
            ledgers_dir,
            backups_dir,
            config_dir,
            state_file,
            retention: retention.unwrap_or(DEFAULT_RETENTION).max(1),
        })
    }

    pub fn new_default() -> Result<Self> {
        Self::new(None, None)
    }

    pub fn ledger_path(&self, name: &str) -> PathBuf {
        self.ledgers_dir
            .join(format!("{}.json", canonical_name(name)))
    }

    fn backup_dir(&self, name: &str) -> PathBuf {
        self.backups_dir.join(canonical_name(name))
    }

    pub fn last_ledger(&self) -> Result<Option<String>> {
        let state = self.read_state()?;
        Ok(state.last_ledger)
    }

    pub fn record_last_ledger(&self, name: Option<&str>) -> Result<()> {
        let mut state = self.read_state()?;
        state.last_ledger = name.map(canonical_name);
        let data = serde_json::to_string_pretty(&state)?;
        write_atomic(&self.state_file, &data)?;
        Ok(())
    }

    fn read_state(&self) -> Result<StoreState> {
        if self.state_file.exists() {
            let data = fs::read_to_string(&self.state_file)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(StoreState::default())
        }
    }

    pub fn create_config_backup(&self, snapshot: &ConfigSnapshot) -> Result<PathBuf> {
        ensure_dir(&self.config_dir)?;
        let file_name = format!(
            "config_{}.json",
            snapshot.created_at.format("%Y-%m-%dT%H-%M-%S")
        );
        let path = self.config_dir.join(file_name);
        let json = serde_json::to_string_pretty(snapshot)?;
        write_atomic(&path, &json)?;
        Ok(path)
    }

    pub fn list_config_backups(&self) -> Result<Vec<ConfigBackupInfo>> {
        if !self.config_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let contents = match fs::read_to_string(&path) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let snapshot: ConfigSnapshot = match serde_json::from_str(&contents) {
                Ok(snapshot) => snapshot,
                Err(_) => continue,
            };
            entries.push(ConfigBackupInfo {
                path,
                created_at: snapshot.created_at,
                note: snapshot.note.clone(),
            });
        }
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(entries)
    }

    pub fn load_config_snapshot(&self, path: &Path) -> Result<ConfigSnapshot> {
        if !path.exists() {
            return Err(BudgetError::StorageError(format!(
                "configuration backup `{}` not found",
                path.display()
            )));
        }
        let data = fs::read_to_string(path)?;
        let snapshot: ConfigSnapshot = serde_json::from_str(&data)?;
        if snapshot.schema_version > CONFIG_BACKUP_SCHEMA_VERSION {
            return Err(BudgetError::StorageError(format!(
                "configuration backup `{}` is from a newer schema version",
                path.display()
            )));
        }
        Ok(snapshot)
    }

    pub fn save_active_config(&self, config: &ConfigData) -> Result<()> {
        let path = PathResolver::config_file_in(&self.root);
        let json = serde_json::to_string_pretty(config)?;
        if let Some(parent) = path.parent() {
            ensure_dir(parent)?;
        }
        write_atomic(&path, &json)?;
        Ok(())
    }

    pub fn base_dir(&self) -> &Path {
        &self.root
    }

    pub fn load_from_path(&self, path: &Path) -> Result<Ledger> {
        load_ledger_from_path(path)
    }

    pub fn save_to_path(&self, ledger: &Ledger, path: &Path) -> Result<()> {
        if path.starts_with(&self.ledgers_dir) {
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                self.backup_existing_file(stem, path)?;
            }
        }
        save_ledger_to_path(ledger, path)
    }

    fn write_backup_file(&self, ledger: &Ledger, name: &str, note: Option<&str>) -> Result<()> {
        let dir = self.backup_dir(name);
        ensure_dir(&dir)?;
        let timestamp = Utc::now().format(BACKUP_TIMESTAMP_FORMAT).to_string();
        let mut file_stem = format!("{}_{}", canonical_name(name), timestamp);
        if let Some(label) = sanitize_backup_note(note) {
            file_stem.push('_');
            file_stem.push_str(&label);
        }
        let path = dir.join(format!("{}.{}", file_stem, BACKUP_EXTENSION));
        let json = serde_json::to_string_pretty(ledger)?;
        write_atomic(&path, &json)?;
        self.prune_backups(name)?;
        Ok(())
    }

    fn backup_existing_file(&self, name: &str, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let dir = self.backup_dir(name);
        ensure_dir(&dir)?;
        let timestamp = Utc::now().format(BACKUP_TIMESTAMP_FORMAT).to_string();
        let backup_name = format!(
            "{}_{}.{}",
            canonical_name(name),
            timestamp,
            BACKUP_EXTENSION
        );
        let backup_path = dir.join(&backup_name);
        fs::copy(path, &backup_path)?;
        self.prune_backups(name)?;
        Ok(())
    }

    fn prune_backups(&self, name: &str) -> Result<()> {
        let backups = self.list_backups(name)?;
        if backups.len() <= self.retention {
            return Ok(());
        }
        for entry in backups.iter().skip(self.retention) {
            let path = self.backup_path(name, entry);
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    pub fn backup_path(&self, name: &str, backup_name: &str) -> PathBuf {
        self.backup_dir(name).join(backup_name)
    }
}

impl StorageBackend for JsonStorage {
    fn save(&self, ledger: &Ledger, name: &str) -> Result<()> {
        let path = self.ledger_path(name);
        if let Some(parent) = path.parent() {
            ensure_dir(parent)?;
        }
        if path.exists() {
            self.backup_existing_file(name, &path)?;
        }
        let json = serde_json::to_string_pretty(ledger)?;
        let tmp = tmp_path(&path);
        write_atomic(&tmp, &json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn load(&self, name: &str) -> Result<Ledger> {
        let path = self.ledger_path(name);
        load_ledger_from_path(&path)
    }

    fn list_backups(&self, name: &str) -> Result<Vec<String>> {
        let dir = self.backup_dir(name);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(BACKUP_EXTENSION) {
                continue;
            }
            let file_name = match path.file_name().and_then(|stem| stem.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            entries.push(file_name);
        }
        entries.sort_by(|a, b| parse_backup_timestamp(b).cmp(&parse_backup_timestamp(a)));
        Ok(entries)
    }

    fn backup(&self, ledger: &Ledger, name: &str, note: Option<&str>) -> Result<()> {
        self.write_backup_file(ledger, name, note)
    }

    fn restore(&self, name: &str, backup_name: &str) -> Result<Ledger> {
        let backup_path = self.backup_path(name, backup_name);
        if !backup_path.exists() {
            return Err(BudgetError::StorageError(format!(
                "backup `{}` not found",
                backup_name
            )));
        }
        let target = self.ledger_path(name);
        fs::copy(&backup_path, &target)?;
        load_ledger_from_path(&target)
    }
}

pub fn save_ledger_to_path(ledger: &Ledger, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let json = serde_json::to_string_pretty(ledger)?;
    let tmp = tmp_path(path);
    write_atomic(&tmp, &json)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load_ledger_from_path(path: &Path) -> Result<Ledger> {
    let data = fs::read_to_string(path)?;
    let ledger: Ledger = serde_json::from_str(&data)?;
    Ok(ledger)
}

#[derive(Debug, Clone)]
pub struct ConfigBackupInfo {
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub config: ConfigData,
}

impl ConfigSnapshot {
    pub fn new(config: ConfigData, note: Option<String>) -> Self {
        Self {
            schema_version: CONFIG_BACKUP_SCHEMA_VERSION,
            created_at: Utc::now(),
            note,
            config,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigData {
    pub base_currency: String,
    pub locale: LocaleConfig,
    pub currency_display: CurrencyDisplay,
    pub negative_style: NegativeStyle,
    pub screen_reader_mode: bool,
    pub high_contrast_mode: bool,
    pub valuation_policy: ValuationPolicy,
}

impl ConfigData {
    pub fn from_ledger(ledger: &Ledger) -> Self {
        Self {
            base_currency: ledger.base_currency.as_str().to_string(),
            locale: ledger.locale.clone(),
            currency_display: ledger.format.currency_display,
            negative_style: ledger.format.negative_style,
            screen_reader_mode: ledger.format.screen_reader_mode,
            high_contrast_mode: ledger.format.high_contrast_mode,
            valuation_policy: ledger.valuation_policy.clone(),
        }
    }

    pub fn apply_to_ledger(&self, ledger: &mut Ledger) {
        ledger.base_currency = CurrencyCode::new(self.base_currency.clone());
        ledger.locale = self.locale.clone();
        ledger.format.currency_display = self.currency_display;
        ledger.format.negative_style = self.negative_style;
        ledger.format.screen_reader_mode = self.screen_reader_mode;
        ledger.format.high_contrast_mode = self.high_contrast_mode;
        ledger.valuation_policy = self.valuation_policy.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::BudgetPeriod;
    use tempfile::TempDir;

    fn storage_with_temp_dir() -> (JsonStorage, TempDir) {
        let temp = TempDir::new().expect("temp dir");
        let storage =
            JsonStorage::new(Some(temp.path().to_path_buf()), Some(3)).expect("json storage");
        (storage, temp)
    }

    fn sample_ledger() -> Ledger {
        Ledger::new("Sample", BudgetPeriod::monthly())
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (storage, _guard) = storage_with_temp_dir();
        let ledger = sample_ledger();
        storage.save(&ledger, "household").expect("save ledger");
        let loaded = storage.load("household").expect("load ledger");
        assert_eq!(loaded.name, "Sample");
    }

    #[test]
    fn backup_writes_timestamped_files() {
        let (storage, _guard) = storage_with_temp_dir();
        let ledger = sample_ledger();
        storage.save(&ledger, "family").expect("save ledger");
        storage
            .backup(&ledger, "family", Some("monthly"))
            .expect("create backup");
        let backups = storage.list_backups("family").expect("list backups");
        assert!(
            !backups.is_empty(),
            "expected at least one backup file to be created"
        );
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreState {
    last_ledger: Option<String>,
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

fn sanitize_backup_note(note: Option<&str>) -> Option<String> {
    let raw = note?.trim();
    if raw.is_empty() {
        return None;
    }
    let mut sanitized = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch.is_whitespace() || matches!(ch, '-' | '.') {
            if !sanitized.is_empty() && !last_dash {
                sanitized.push('-');
                last_dash = true;
            }
        }
    }
    let trimmed = sanitized.trim_matches('-').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn parse_backup_timestamp(name: &str) -> Option<DateTime<Utc>> {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() < 3 {
        return None;
    }
    let date_part = parts.get(parts.len() - 2)?;
    let time_part = parts.last()?;
    if !is_digits(date_part, 8) || !time_part.ends_with(".json") {
        return None;
    }
    let time_digits = &time_part[..time_part.len() - 5];
    if !is_digits(&time_digits, 4) {
        return None;
    }
    let raw = format!("{}{}", date_part, time_digits);
    NaiveDateTime::parse_from_str(&raw, "%Y%m%d%H%M")
        .ok()
        .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
}

fn is_digits(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_digit())
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

fn write_atomic(path: &Path, data: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub fn ledger_warnings(ledger: &Ledger) -> Vec<String> {
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
