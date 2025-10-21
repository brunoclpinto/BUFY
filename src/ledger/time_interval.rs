use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimeUnit {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeInterval {
    pub every: u32,
    pub unit: TimeUnit,
}

impl TimeInterval {
    pub fn next_date(&self, from: NaiveDate) -> NaiveDate {
        match self.unit {
            TimeUnit::Day => from + Duration::days(self.every as i64),
            TimeUnit::Week => from + Duration::weeks(self.every as i64),
            TimeUnit::Month => {
                let mut year = from.year();
                let mut month = from.month() as i32 + self.every as i32;
                while month > 12 {
                    month -= 12;
                    year += 1;
                }
                NaiveDate::from_ymd_opt(year, month as u32, from.day()).unwrap_or(from)
            }
            TimeUnit::Year => {
                NaiveDate::from_ymd_opt(from.year() + self.every as i32, from.month(), from.day())
                    .unwrap_or(from)
            }
        }
    }

    pub fn label(&self) -> String {
        match (self.every, &self.unit) {
            (1, TimeUnit::Day) => "Daily".into(),
            (1, TimeUnit::Week) => "Weekly".into(),
            (1, TimeUnit::Month) => "Monthly".into(),
            (1, TimeUnit::Year) => "Yearly".into(),
            (n, unit) => format!("Every {} {:?}{}", n, unit, if n > 1 { "s" } else { "" }),
        }
    }
}
