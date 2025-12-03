//! Shared traits, time utilities, and enums for budgeting primitives.

use std::fmt;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Exposes a stable identifier for entities stored in the ledger.
pub trait Identifiable {
    fn id(&self) -> Uuid;
}

/// Provides read-only access to an entity's display name.
pub trait NamedEntity {
    fn name(&self) -> &str;
}

/// Associates entities with optional category ownership.
pub trait BelongsToCategory {
    fn category_id(&self) -> Option<Uuid>;
}

/// Supplies a common contract for retrieving numeric amounts.
pub trait Amounted {
    fn amount(&self) -> f64;
}

/// Converts an entity into a user-facing display label.
pub trait Displayable {
    fn display_label(&self) -> String;
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// Enumerates canonical budgeting cadences.
#[derive(Default)]
pub enum BudgetPeriod {
    Daily,
    Weekly,
    #[default]
    Monthly,
    Yearly,
    Custom(u32),
}

impl BudgetPeriod {
    /// Returns the nominal day-count representation for the period.
    pub fn days(self) -> Option<u32> {
        match self {
            BudgetPeriod::Daily => Some(1),
            BudgetPeriod::Weekly => Some(7),
            BudgetPeriod::Monthly => Some(30),
            BudgetPeriod::Yearly => Some(365),
            BudgetPeriod::Custom(value) => Some(value.max(1)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Enumerates time units used by `TimeInterval`.
pub enum TimeUnit {
    Day,
    Week,
    Month,
    Year,
}

impl fmt::Display for TimeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            TimeUnit::Day => "Day",
            TimeUnit::Week => "Week",
            TimeUnit::Month => "Month",
            TimeUnit::Year => "Year",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Represents a time unit and multiplier for recurrence calculations.
pub struct TimeInterval {
    pub every: u32,
    pub unit: TimeUnit,
}

impl TimeInterval {
    /// Calculates the next date after `from` according to the interval.
    pub fn next_date(&self, from: NaiveDate) -> NaiveDate {
        match self.unit {
            TimeUnit::Day => from + Duration::days(self.every as i64),
            TimeUnit::Week => from + Duration::weeks(self.every as i64),
            TimeUnit::Month => shift_month(from, self.every as i32),
            TimeUnit::Year => shift_year(from, self.every as i32),
        }
    }

    pub fn previous_date(&self, from: NaiveDate) -> NaiveDate {
        match self.unit {
            TimeUnit::Day => from - Duration::days(self.every as i64),
            TimeUnit::Week => from - Duration::weeks(self.every as i64),
            TimeUnit::Month => shift_month(from, -(self.every as i32)),
            TimeUnit::Year => shift_year(from, -(self.every as i32)),
        }
    }

    pub fn add_to(&self, from: NaiveDate, steps: i32) -> NaiveDate {
        if steps == 0 {
            return from;
        }
        if steps > 0 {
            (0..steps).fold(from, |date, _| self.next_date(date))
        } else {
            (0..(-steps)).fold(from, |date, _| self.previous_date(date))
        }
    }

    pub fn normalize_anchor(&self, date: NaiveDate) -> NaiveDate {
        match self.unit {
            TimeUnit::Day => date,
            TimeUnit::Week => {
                let delta = date.weekday().num_days_from_monday() as i64;
                date - Duration::days(delta)
            }
            TimeUnit::Month => {
                let base = date.with_day(1).unwrap();
                if self.every <= 1 {
                    base
                } else {
                    align_month_to_interval(base, self.every)
                }
            }
            TimeUnit::Year => {
                let base = NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap();
                if self.every <= 1 {
                    base
                } else {
                    align_year_to_interval(base, self.every)
                }
            }
        }
    }

    pub fn cycle_start(&self, anchor: NaiveDate, reference: NaiveDate) -> NaiveDate {
        match self.unit {
            TimeUnit::Day => cycle_start_linear(anchor, reference, self.every as i64),
            TimeUnit::Week => cycle_start_linear(anchor, reference, (self.every * 7) as i64),
            TimeUnit::Month => cycle_start_months(anchor, reference, self.every as i32),
            TimeUnit::Year => cycle_start_years(anchor, reference, self.every as i32),
        }
    }

    pub fn label(&self) -> String {
        match (self.every, &self.unit) {
            (1, TimeUnit::Day) => "Daily".into(),
            (1, TimeUnit::Week) => "Weekly".into(),
            (1, TimeUnit::Month) => "Monthly".into(),
            (1, TimeUnit::Year) => "Yearly".into(),
            (n, unit) => format!("Every {} {}{}", n, unit, if n > 1 { "s" } else { "" }),
        }
    }
}

impl fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            BudgetPeriod::Daily => "Daily",
            BudgetPeriod::Weekly => "Weekly",
            BudgetPeriod::Monthly => "Monthly",
            BudgetPeriod::Yearly => "Yearly",
            BudgetPeriod::Custom(value) => return write!(f, "Custom({value})"),
        };
        f.write_str(label)
    }
}

fn cycle_start_linear(anchor: NaiveDate, reference: NaiveDate, interval_days: i64) -> NaiveDate {
    let diff = reference - anchor;
    let steps = diff.num_days().div_euclid(interval_days);
    anchor + Duration::days(steps * interval_days)
}

fn cycle_start_months(anchor: NaiveDate, reference: NaiveDate, interval_months: i32) -> NaiveDate {
    let anchor_idx = anchor.year() * 12 + anchor.month() as i32 - 1;
    let reference_idx = reference.year() * 12 + reference.month() as i32 - 1;
    let diff = reference_idx - anchor_idx;
    let steps = diff.div_euclid(interval_months);
    let mut start = shift_month(anchor, steps * interval_months);
    start = start.with_day(1).unwrap();
    if start > reference {
        start = shift_month(start, -interval_months);
    }
    start
}

fn cycle_start_years(anchor: NaiveDate, reference: NaiveDate, interval_years: i32) -> NaiveDate {
    let diff = reference.year() - anchor.year();
    let steps = diff.div_euclid(interval_years);
    let mut start = shift_year(anchor, steps * interval_years);
    start = start.with_month(1).unwrap().with_day(1).unwrap();
    if start > reference {
        start = shift_year(start, -interval_years);
    }
    start
}

fn shift_month(date: NaiveDate, months: i32) -> NaiveDate {
    let mut year = date.year();
    let mut month = date.month() as i32 + months;
    let mut day = date.day();
    while month > 12 {
        month -= 12;
        year += 1;
    }
    while month < 1 {
        month += 12;
        year -= 1;
    }
    day = day.min(days_in_month(year, month as u32));
    NaiveDate::from_ymd_opt(year, month as u32, day).unwrap()
}

fn shift_year(date: NaiveDate, years: i32) -> NaiveDate {
    let year = date.year() + years;
    let mut day = date.day();
    let month = date.month();
    day = day.min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month, 28).unwrap());
    let last_current = first_next - Duration::days(1);
    last_current.day()
}

fn align_month_to_interval(date: NaiveDate, interval: u32) -> NaiveDate {
    let month_index = date.month() - 1;
    let block = (month_index / interval) * interval;
    let month = block + 1;
    NaiveDate::from_ymd_opt(date.year(), month, 1).unwrap()
}

fn align_year_to_interval(date: NaiveDate, interval: u32) -> NaiveDate {
    let year = date.year();
    let offset = (year - 1).rem_euclid(interval as i32);
    let aligned_year = year - offset;
    NaiveDate::from_ymd_opt(aligned_year, 1, 1).unwrap()
}
