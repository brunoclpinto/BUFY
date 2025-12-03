use std::{
    cmp::Reverse,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};

use crate::{Config, ConfigError};

pub const CONFIG_BACKUP_SCHEMA_VERSION: u32 = 1;
const BACKUP_EXTENSION: &str = "json";
const BACKUP_TIMESTAMP_FORMAT: &str = "%Y%m%d_%H%M";
const TMP_SUFFIX: &str = "tmp";

/// Handles persistence and backup management for [`Config`].
#[derive(Debug, Clone)]
pub struct ConfigManager {
    config_path: PathBuf,
    backups_dir: PathBuf,
}

impl ConfigManager {
    pub fn new(config_path: PathBuf, backups_dir: PathBuf) -> Self {
        Self {
            config_path,
            backups_dir,
        }
    }

    pub fn with_base_dir(base: PathBuf) -> Result<Self, ConfigError> {
        fs::create_dir_all(&base)?;
        let config_dir = base.join("config");
        fs::create_dir_all(&config_dir)?;
        let backups_dir = config_dir.join("backups");
        fs::create_dir_all(&backups_dir)?;
        let config_path = config_dir.join("config.json");
        Ok(Self::new(config_path, backups_dir))
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn backups_dir(&self) -> &Path {
        &self.backups_dir
    }

    pub fn load(&self) -> Result<Config, ConfigError> {
        if self.config_path.exists() {
            let data = fs::read_to_string(&self.config_path)?;
            serde_json::from_str(&data).map_err(|err| ConfigError::Serde(err.to_string()))
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(config)
            .map_err(|err| ConfigError::Serde(err.to_string()))?;
        let tmp = tmp_path(&self.config_path);
        write_atomic(&tmp, &json)?;
        fs::rename(&tmp, &self.config_path)?;
        Ok(())
    }

    pub fn backup(&self, config: &Config, note: Option<&str>) -> Result<String, ConfigError> {
        fs::create_dir_all(&self.backups_dir)?;
        let timestamp = Utc::now().format(BACKUP_TIMESTAMP_FORMAT).to_string();
        let mut name = format!("config_{}", timestamp);
        if let Some(label) = sanitize_note(note) {
            name.push('_');
            name.push_str(&label);
        }
        name.push_str(&format!(".{}", BACKUP_EXTENSION));
        let path = self.backups_dir.join(&name);
        let json = serde_json::to_string_pretty(config)
            .map_err(|err| ConfigError::Serde(err.to_string()))?;
        write_atomic(&path, &json)?;
        Ok(name)
    }

    pub fn restore(&self, backup_name: &str) -> Result<Config, ConfigError> {
        let path = self.backups_dir.join(backup_name);
        if !path.exists() {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("configuration backup `{}` not found", backup_name),
            )));
        }
        let data = fs::read_to_string(&path)?;
        serde_json::from_str(&data).map_err(|err| ConfigError::Serde(err.to_string()))
    }

    pub fn list_backups(&self) -> Result<Vec<String>, ConfigError> {
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

fn write_atomic(path: &Path, data: &str) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}
