use chrono::{DateTime, NaiveDate, Utc};

use bufy_core::Clock;

/// Real-time clock backed by the system UTC time source.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn today(&self) -> NaiveDate {
        self.now().date_naive()
    }
}
