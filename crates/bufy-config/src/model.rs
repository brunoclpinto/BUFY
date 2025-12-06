use serde::{de::Deserializer, Deserialize, Serialize};
use std::{fmt, path::PathBuf};

/// Stores user-configurable CLI preferences and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub locale: String,
    pub currency: String,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub accessibility: AccessibilitySettings,
    #[serde(default = "Config::default_ui_color_enabled")]
    pub ui_color_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_opened_ledger: Option<String>,
    #[serde(default)]
    pub audio_feedback: bool,
    #[serde(default = "Config::default_budget_period_value")]
    pub default_budget_period: String,
    #[serde(default)]
    pub default_currency_precision: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional custom root directory for ledgers. Defaults to `~/Documents/Ledgers`.
    pub default_ledger_root: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional custom root directory for backups. Defaults to `~/Documents/Ledger`.
    pub default_backup_root: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            currency: "USD".into(),
            theme: Theme::default(),
            accessibility: AccessibilitySettings::default(),
            ui_color_enabled: Self::default_ui_color_enabled(),
            last_opened_ledger: None,
            audio_feedback: false,
            default_budget_period: Self::default_budget_period_value(),
            default_currency_precision: None,
            default_ledger_root: None,
            default_backup_root: None,
        }
    }
}

impl Config {
    pub fn default_budget_period_value() -> String {
        "monthly".into()
    }

    pub fn default_ui_color_enabled() -> bool {
        true
    }

    pub fn resolve_default_ledger_root(&self) -> PathBuf {
        if let Some(path) = &self.default_ledger_root {
            return path.clone();
        }

        let base = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        base.join("Ledgers")
    }

    pub fn resolve_default_backup_root(&self) -> PathBuf {
        if let Some(path) = &self.default_backup_root {
            return path.clone();
        }

        let base = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        base.join("Ledger")
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Plain,
    Iconic,
}

impl Theme {
    fn from_value(value: Option<String>) -> Self {
        value
            .map(|v| Theme::from_str(v.trim()))
            .unwrap_or_else(Theme::default)
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "plain" => Theme::Plain,
            _ => Theme::Iconic,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Iconic
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Theme::Plain => "plain",
            Theme::Iconic => "iconic",
        };
        f.write_str(label)
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        Ok(Theme::from_value(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilitySettings {
    #[serde(default)]
    pub plain_output: bool,
    #[serde(default)]
    pub high_contrast: bool,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            plain_output: false,
            high_contrast: false,
        }
    }
}
