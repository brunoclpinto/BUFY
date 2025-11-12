//! Manages CLI configuration persistence, backups, and restores.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use crate::core::{
    errors::BudgetError,
    utils::{ensure_dir, PathResolver},
};

const BACKUP_EXTENSION: &str = "json";
const BACKUP_TIMESTAMP_FORMAT: &str = "%Y%m%d_%H%M";
const TMP_SUFFIX: &str = "tmp";

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Stores user-configurable CLI preferences and metadata.
pub struct Config {
    pub locale: String,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_opened_ledger: Option<String>,
    #[serde(default)]
    pub audio_feedback: bool,
    #[serde(default = "Config::default_budget_period_value")]
    pub default_budget_period: String,
    #[serde(default)]
    pub default_currency_precision: Option<u8>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            currency: "USD".into(),
            theme: None,
            last_opened_ledger: None,
            audio_feedback: false,
            default_budget_period: Self::default_budget_period_value(),
            default_currency_precision: None,
        }
    }
}

impl Config {
    pub fn default_budget_period_value() -> String {
        "monthly".into()
    }
}

/// Handles persistence and backup management for [`Config`].
///
/// See also: [`crate::storage::json_backend::JsonStorage`] for ledger persistence.
pub struct ConfigManager {
    path: PathBuf,
    backups_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self, BudgetError> {
        Self::from_base(PathResolver::base_dir())
    }

    pub fn with_base_dir(base: PathBuf) -> Result<Self, BudgetError> {
        Self::from_base(base)
    }

    fn from_base(base: PathBuf) -> Result<Self, BudgetError> {
        ensure_dir(&base)?;
        let config_root = PathResolver::config_dir_in(&base);
        ensure_dir(&config_root)?;
        let backups_dir = PathResolver::config_backup_dir_in(&base);
        ensure_dir(&backups_dir)?;
        Ok(Self {
            path: PathResolver::config_file_in(&base),
            backups_dir,
        })
    }

    pub fn load(&self) -> Result<Config, BudgetError> {
        if self.path.exists() {
            let data = fs::read_to_string(&self.path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self, config: &Config) -> Result<(), BudgetError> {
        if let Some(parent) = self.path.parent() {
            ensure_dir(parent)?;
        }
        let json = serde_json::to_string_pretty(config)?;
        let tmp = tmp_path(&self.path);
        write_atomic(&tmp, &json)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn backup(&self, config: &Config, note: Option<&str>) -> Result<String, BudgetError> {
        ensure_dir(&self.backups_dir)?;
        let timestamp = Utc::now().format(BACKUP_TIMESTAMP_FORMAT).to_string();
        let mut name = format!("config_{}", timestamp);
        if let Some(label) = sanitize_note(note) {
            name.push('_');
            name.push_str(&label);
        }
        name.push_str(&format!(".{}", BACKUP_EXTENSION));
        let path = self.backups_dir.join(&name);
        let json = serde_json::to_string_pretty(config)?;
        write_atomic(&path, &json)?;
        Ok(name)
    }

    pub fn restore(&self, backup_name: &str) -> Result<Config, BudgetError> {
        let path = self.backups_dir.join(backup_name);
        if !path.exists() {
            return Err(BudgetError::StorageError(format!(
                "configuration backup `{}` not found",
                backup_name
            )));
        }
        let data = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    pub fn list_backups(&self) -> Result<Vec<String>, BudgetError> {
        if !self.backups_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.backups_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(BACKUP_EXTENSION) {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                entries.push(name.to_string());
            }
        }
        entries.sort_by_key(|name| Reverse(parse_timestamp(name)));
        Ok(entries)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn sanitize_note(note: Option<&str>) -> Option<String> {
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

fn parse_timestamp(name: &str) -> Option<DateTime<Utc>> {
    let trimmed = name.strip_suffix(&format!(".{}", BACKUP_EXTENSION))?;
    let segments: Vec<&str> = trimmed.split('_').collect();
    if segments.len() < 2 {
        return None;
    }
    let time_part = segments.last()?;
    let date_part = segments.get(segments.len() - 2)?;
    if date_part.len() != 8 || time_part.len() != 4 {
        return None;
    }
    let raw = format!("{}{}", date_part, time_part);
    chrono::NaiveDateTime::parse_from_str(&raw, "%Y%m%d%H%M")
        .ok()
        .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
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

fn write_atomic(path: &Path, data: &str) -> Result<(), BudgetError> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn manager() -> (ConfigManager, TempDir) {
        let temp = TempDir::new().expect("temp dir");
        let manager =
            ConfigManager::with_base_dir(temp.path().to_path_buf()).expect("config manager");
        (manager, temp)
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (manager, _guard) = manager();
        let mut config = Config::default();
        config.locale = "en-GB".into();
        config.currency = "EUR".into();
        manager.save(&config).expect("save config");
        let loaded = manager.load().expect("load config");
        assert_eq!(loaded.locale, "en-GB");
        assert_eq!(loaded.currency, "EUR");
    }

    #[test]
    fn backup_and_restore_cycle() {
        let (manager, _guard) = manager();
        let mut config = Config::default();
        config.theme = Some("dark".into());
        manager.save(&config).expect("save config");
        let backup_name = manager
            .backup(&config, Some("smoke"))
            .expect("create backup");

        config.theme = Some("light".into());
        manager.save(&config).expect("save modified config");

        let restored = manager.restore(&backup_name).expect("restore backup");
        assert_eq!(restored.theme.as_deref(), Some("dark"));
    }
}
