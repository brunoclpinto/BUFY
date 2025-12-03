use std::{
    cmp::Reverse,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use bufy_core::{
    storage::{LedgerBackupInfo, LedgerStorage},
    BudgetService, CoreError,
};
use bufy_domain::{Ledger, LedgerBudgetPeriod};
use chrono::{DateTime, NaiveDateTime, Utc};

const BACKUP_EXTENSION: &str = "json";
const BACKUP_TIMESTAMP_FORMAT: &str = "%Y%m%d_%H%M";
const TMP_SUFFIX: &str = "tmp";
const DEFAULT_RETENTION: usize = 5;

/// Filesystem-backed JSON persistence for ledgers and their backups.
#[derive(Clone)]
pub struct JsonLedgerStorage {
    ledgers_dir: PathBuf,
    backups_dir: PathBuf,
    retention: usize,
}

impl JsonLedgerStorage {
    pub fn new(ledgers_dir: PathBuf, backups_dir: PathBuf) -> Result<Self, CoreError> {
        Self::with_retention(ledgers_dir, backups_dir, DEFAULT_RETENTION)
    }

    pub fn with_retention(
        ledgers_dir: PathBuf,
        backups_dir: PathBuf,
        retention: usize,
    ) -> Result<Self, CoreError> {
        fs::create_dir_all(&ledgers_dir)?;
        fs::create_dir_all(&backups_dir)?;
        Ok(Self {
            ledgers_dir,
            backups_dir,
            retention: retention.max(1),
        })
    }

    pub fn ledger_path(&self, name: &str) -> PathBuf {
        self.ledgers_dir
            .join(format!("{}.{}", canonical_name(name), BACKUP_EXTENSION))
    }

    pub fn backup_path(&self, name: &str, backup: &str) -> PathBuf {
        self.backup_dir(name).join(backup)
    }

    pub fn list_ledger_metadata(&self) -> Result<Vec<LedgerMetadata>, CoreError> {
        let mut entries = Vec::new();
        for slug in self.list_ledgers()? {
            let ledger = self.load_ledger(&slug)?;
            let summary = BudgetService::summarize_current_period(&ledger);
            let path = self.ledger_path(&slug);
            entries.push(LedgerMetadata {
                slug: slug.clone(),
                name: ledger.name.clone(),
                path,
                created_at: ledger.created_at,
                updated_at: ledger.updated_at,
                budget_period: ledger.budget_period.clone(),
                account_count: ledger.accounts.len(),
                category_count: ledger.categories.len(),
                transaction_count: ledger.transactions.len(),
                simulation_count: ledger.simulations.len(),
                total_budgeted: summary.totals.budgeted,
                total_available: summary.totals.remaining,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn list_backup_metadata(&self, name: &str) -> Result<Vec<BackupMetadata>, CoreError> {
        let backups = self.list_backups(name)?;
        let mut rows = Vec::new();
        for entry in backups {
            let size_bytes = fs::metadata(&entry.path)
                .map(|meta| meta.len())
                .unwrap_or(0);
            rows.push(BackupMetadata {
                name: entry.id.clone(),
                created_at: parse_backup_timestamp(&entry.id),
                size_bytes,
                path: entry.path.clone(),
            });
        }
        rows.sort_by_key(|meta| Reverse(meta.created_at));
        Ok(rows)
    }

    pub fn save_to_path(&self, ledger: &Ledger, path: &Path) -> Result<(), CoreError> {
        if path.starts_with(&self.ledgers_dir) {
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                self.backup_existing_file(stem, path)?;
            }
        }
        save_ledger_to_path(ledger, path)
    }

    pub fn load_from_path(&self, path: &Path) -> Result<Ledger, CoreError> {
        load_ledger_from_path(path)
    }

    pub fn delete_backup(&self, name: &str, backup_id: &str) -> Result<(), CoreError> {
        let path = self.backup_path(name, backup_id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn backup_dir(&self, name: &str) -> PathBuf {
        self.backups_dir.join(canonical_name(name))
    }

    fn write_backup_file(
        &self,
        ledger: &Ledger,
        name: &str,
        note: Option<&str>,
    ) -> Result<LedgerBackupInfo, CoreError> {
        let dir = self.backup_dir(name);
        fs::create_dir_all(&dir)?;
        let timestamp = Utc::now().format(BACKUP_TIMESTAMP_FORMAT).to_string();
        let mut stem = format!("{}_{}", canonical_name(name), timestamp);
        if let Some(label) = sanitize_backup_note(note) {
            stem.push('_');
            stem.push_str(&label);
        }
        let file_name = format!("{}.{}", stem, BACKUP_EXTENSION);
        let path = dir.join(&file_name);
        write_atomic(&path, &serialize_ledger(ledger)?)?;
        self.prune_backups(name)?;
        Ok(LedgerBackupInfo {
            ledger: canonical_name(name),
            id: file_name.clone(),
            created_at: timestamp,
            path,
        })
    }

    fn backup_existing_file(&self, name: &str, path: &Path) -> Result<(), CoreError> {
        if !path.exists() {
            return Ok(());
        }
        let dir = self.backup_dir(name);
        fs::create_dir_all(&dir)?;
        let timestamp = Utc::now().format(BACKUP_TIMESTAMP_FORMAT).to_string();
        let file_name = format!(
            "{}_{}.{}",
            canonical_name(name),
            timestamp,
            BACKUP_EXTENSION
        );
        let backup_path = dir.join(&file_name);
        fs::copy(path, &backup_path)?;
        self.prune_backups(name)?;
        Ok(())
    }

    fn prune_backups(&self, name: &str) -> Result<(), CoreError> {
        let mut entries = self.list_backups(name)?;
        entries.sort_by_key(|info| Reverse(parse_backup_timestamp(&info.id)));
        for entry in entries.into_iter().skip(self.retention) {
            let _ = fs::remove_file(entry.path);
        }
        Ok(())
    }
}

impl LedgerStorage for JsonLedgerStorage {
    fn save_ledger(&self, name: &str, ledger: &Ledger) -> Result<(), CoreError> {
        let path = self.ledger_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            self.backup_existing_file(name, &path)?;
        }
        let tmp = tmp_path(&path);
        write_atomic(&tmp, &serialize_ledger(ledger)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn load_ledger(&self, name: &str) -> Result<Ledger, CoreError> {
        load_ledger_from_path(&self.ledger_path(name))
    }

    fn list_ledgers(&self) -> Result<Vec<String>, CoreError> {
        if !self.ledgers_dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.ledgers_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some(BACKUP_EXTENSION) {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    fn delete_ledger(&self, name: &str) -> Result<(), CoreError> {
        let path = self.ledger_path(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn save_ledger_to_path(&self, ledger: &Ledger, path: &Path) -> Result<(), CoreError> {
        self.save_to_path(ledger, path)
    }

    fn load_ledger_from_path(&self, path: &Path) -> Result<Ledger, CoreError> {
        self.load_from_path(path)
    }

    fn backup_ledger(
        &self,
        name: &str,
        ledger: &Ledger,
        note: Option<&str>,
    ) -> Result<LedgerBackupInfo, CoreError> {
        self.write_backup_file(ledger, name, note)
    }

    fn list_backups(&self, name: &str) -> Result<Vec<LedgerBackupInfo>, CoreError> {
        let dir = self.backup_dir(name);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        let ledger_slug = canonical_name(name);
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(BACKUP_EXTENSION) {
                continue;
            }
            if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                entries.push(LedgerBackupInfo {
                    ledger: ledger_slug.clone(),
                    id: file_name.to_string(),
                    created_at: file_name.to_string(),
                    path: path.clone(),
                });
            }
        }
        entries.sort_by_key(|info| Reverse(parse_backup_timestamp(&info.id)));
        Ok(entries)
    }

    fn restore_backup(&self, backup: &LedgerBackupInfo) -> Result<Ledger, CoreError> {
        if !backup.path.exists() {
            return Err(CoreError::Storage(format!(
                "backup `{}` not found",
                backup.id
            )));
        }
        let target = self.ledger_path(&backup.ledger);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&backup.path, &target)?;
        load_ledger_from_path(&target)
    }
}

/// Saves a ledger to an arbitrary path on disk.
pub fn save_ledger_to_path(ledger: &Ledger, path: &Path) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path(path);
    write_atomic(&tmp, &serialize_ledger(ledger)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Loads a ledger from the provided filesystem path.
pub fn load_ledger_from_path(path: &Path) -> Result<Ledger, CoreError> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data).map_err(|err| CoreError::Serde(err.to_string()))?)
}

#[derive(Debug, Clone)]
pub struct LedgerMetadata {
    pub slug: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub budget_period: LedgerBudgetPeriod,
    pub account_count: usize,
    pub category_count: usize,
    pub transaction_count: usize,
    pub simulation_count: usize,
    pub total_budgeted: f64,
    pub total_available: f64,
}

#[derive(Debug, Clone)]
pub struct BackupMetadata {
    pub name: String,
    pub created_at: Option<DateTime<Utc>>,
    pub size_bytes: u64,
    pub path: PathBuf,
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
        } else if (ch.is_whitespace() || matches!(ch, '-' | '.'))
            && !sanitized.is_empty()
            && !last_dash
        {
            sanitized.push('-');
            last_dash = true;
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
    let trimmed = name.strip_suffix(&format!(".{}", BACKUP_EXTENSION))?;
    let mut segments = trimmed.split('_').collect::<Vec<_>>();
    if segments.len() < 2 {
        return None;
    }
    let time = segments.pop().unwrap();
    let date = segments.pop().unwrap();
    if !is_digits(date, 8) || !is_digits(time, 4) {
        return None;
    }
    let raw = format!("{}{}", date, time);
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

fn write_atomic(path: &Path, data: &str) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn serialize_ledger(ledger: &Ledger) -> Result<String, CoreError> {
    serde_json::to_string_pretty(ledger).map_err(|err| CoreError::Serde(err.to_string()))
}
