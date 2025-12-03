use serde::{Deserialize, Serialize};

/// Stores user-configurable CLI preferences and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub locale: String,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default = "Config::default_ui_color_enabled")]
    pub ui_color_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_style: Option<String>,
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
            ui_color_enabled: Self::default_ui_color_enabled(),
            ui_style: None,
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

    pub fn default_ui_color_enabled() -> bool {
        true
    }
}
