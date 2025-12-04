use chrono::NaiveDate;

/// Formats currency amounts for presentation.
pub trait CurrencyFormatter: Send + Sync {
    fn format_amount(&self, amount: f64, currency: &str) -> String;
}

/// Formats dates for presentation.
pub trait DateFormatter: Send + Sync {
    fn format_date(&self, date: NaiveDate) -> String;
}
