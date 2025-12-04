use std::sync::{Arc, RwLock};

use bufy_config::Config;
use bufy_core::{CurrencyFormatter, DateFormatter};
use chrono::NaiveDate;

/// Lightweight formatter implementations backed by the active CLI configuration.
#[derive(Clone)]
pub struct CliFormatters {
    config: Arc<RwLock<Config>>,
}

impl CliFormatters {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self { config }
    }

    fn currency_precision(&self, config: &Config) -> usize {
        config
            .default_currency_precision
            .map(|value| value as usize)
            .unwrap_or(2)
    }

    fn date_pattern(&self, locale: &str) -> &'static str {
        match locale {
            "en-GB" | "fr-FR" => "%d/%m/%Y",
            _ => "%Y-%m-%d",
        }
    }
}

impl CurrencyFormatter for CliFormatters {
    fn format_amount(&self, amount: f64, currency: &str) -> String {
        let config = self.config.read().expect("config formatter lock poisoned");
        let code = if currency.is_empty() {
            config.currency.as_str()
        } else {
            currency
        };
        let precision = self.currency_precision(&config);
        format!(
            "{amount:.prec$} {code}",
            amount = amount,
            prec = precision,
            code = code
        )
    }
}

impl DateFormatter for CliFormatters {
    fn format_date(&self, date: NaiveDate) -> String {
        let config = self.config.read().expect("config formatter lock poisoned");
        let pattern = self.date_pattern(&config.locale);
        date.format(pattern).to_string()
    }
}
