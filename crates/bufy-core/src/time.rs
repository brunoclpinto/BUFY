use chrono::{DateTime, NaiveDate, Utc};

/// Clock abstracts access to the current timestamp so services remain deterministic in tests.
pub trait Clock: Send + Sync {
    /// Returns the current UTC timestamp.
    fn now(&self) -> DateTime<Utc>;

    /// Returns the current UTC date. Defaults to `now().date_naive()`.
    fn today(&self) -> NaiveDate {
        self.now().date_naive()
    }
}
