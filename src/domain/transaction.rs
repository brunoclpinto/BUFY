use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::common::{Displayable, Identifiable};

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
            TimeUnit::Month => shift_month(from, self.every as i32),
            TimeUnit::Year => shift_year(from, self.every as i32),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub from_account: Uuid,
    pub to_account: Uuid,
    pub category_id: Option<Uuid>,
    pub scheduled_date: NaiveDate,
    pub actual_date: Option<NaiveDate>,
    pub budgeted_amount: f64,
    pub actual_amount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub recurrence: Option<Recurrence>,
    #[serde(default)]
    pub recurrence_series_id: Option<Uuid>,
    pub status: TransactionStatus,
}

impl Transaction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        from_account: Uuid,
        to_account: Uuid,
        category_id: Option<Uuid>,
        scheduled_date: NaiveDate,
        budgeted_amount: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_account,
            to_account,
            category_id,
            scheduled_date,
            actual_date: None,
            budgeted_amount,
            actual_amount: None,
            currency: None,
            notes: None,
            recurrence: None,
            recurrence_series_id: None,
            status: TransactionStatus::Planned,
        }
    }

    pub fn with_recurrence(mut self, recurrence: Recurrence) -> Self {
        self.set_recurrence(Some(recurrence));
        self
    }

    pub fn set_recurrence(&mut self, mut recurrence: Option<Recurrence>) {
        if let Some(rule) = recurrence.as_mut() {
            if rule.series_id.is_nil() {
                rule.series_id = self.id;
            }
            if rule.start_date != self.scheduled_date {
                rule.start_date = self.scheduled_date;
            }
        }
        self.recurrence_series_id = recurrence.as_ref().map(|r| r.series_id);
        self.recurrence = recurrence;
    }

    pub fn recurrence_series(&self) -> Option<Uuid> {
        self.recurrence_series_id
            .or_else(|| self.recurrence.as_ref().map(|_| self.id))
    }

    pub fn mark_completed(&mut self, actual_date: NaiveDate, actual_amount: f64) {
        self.actual_date = Some(actual_date);
        self.actual_amount = Some(actual_amount);
        self.status = TransactionStatus::Completed;
    }
}

impl Identifiable for Transaction {
    fn id(&self) -> Uuid {
        self.id
    }
}

impl Displayable for Transaction {
    fn display_label(&self) -> String {
        format!("txn:{} [{:?}]", self.id, self.status)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Planned,
    Completed,
    Missed,
    Simulated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recurrence {
    #[serde(default = "Recurrence::default_series_id")]
    pub series_id: Uuid,
    pub start_date: NaiveDate,
    pub interval: TimeInterval,
    #[serde(default)]
    pub mode: RecurrenceMode,
    #[serde(default)]
    pub end: RecurrenceEnd,
    #[serde(default)]
    pub exceptions: Vec<NaiveDate>,
    #[serde(default)]
    pub status: RecurrenceStatus,
    #[serde(default)]
    pub last_generated: Option<NaiveDate>,
    #[serde(default)]
    pub last_completed: Option<NaiveDate>,
    #[serde(default)]
    pub generated_occurrences: u32,
    #[serde(default)]
    pub next_scheduled: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RecurrenceMode {
    /// Follows fixed planned schedule regardless of actual timing.
    #[default]
    FixedSchedule,
    /// Starts next period after the actual performed date.
    AfterLastPerformed,
}

impl Recurrence {
    pub fn new(start_date: NaiveDate, interval: TimeInterval, mode: RecurrenceMode) -> Recurrence {
        Self {
            series_id: Uuid::new_v4(),
            start_date,
            interval,
            mode,
            end: RecurrenceEnd::Never,
            exceptions: Vec::new(),
            status: RecurrenceStatus::Active,
            last_generated: None,
            last_completed: None,
            generated_occurrences: 0,
            next_scheduled: None,
        }
    }

    pub fn ensure_series_id(&mut self, fallback: Uuid) {
        if self.series_id.is_nil() {
            self.series_id = fallback;
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, RecurrenceStatus::Active)
    }

    pub fn is_exception(&self, date: NaiveDate) -> bool {
        self.exceptions.contains(&date)
    }

    pub fn allows_occurrence(&self, occurrence_index: u32, candidate: NaiveDate) -> bool {
        if candidate < self.start_date {
            return false;
        }
        match &self.end {
            RecurrenceEnd::Never => true,
            RecurrenceEnd::OnDate(end_date) => candidate <= *end_date,
            RecurrenceEnd::AfterOccurrences(limit) => occurrence_index < *limit,
        }
    }

    pub fn next_occurrence(
        &self,
        last_scheduled: NaiveDate,
        last_performed: Option<NaiveDate>,
    ) -> NaiveDate {
        let mut candidate = match self.mode {
            RecurrenceMode::FixedSchedule => self.interval.next_date(last_scheduled),
            RecurrenceMode::AfterLastPerformed => {
                if let Some(performed) = last_performed {
                    self.interval.next_date(performed)
                } else {
                    self.interval.next_date(last_scheduled)
                }
            }
        };
        let mut guard = 0usize;
        while self.is_exception(candidate) {
            candidate = self.interval.next_date(candidate);
            guard += 1;
            if guard >= 512 {
                break;
            }
        }
        candidate
    }

    pub fn default_series_id() -> Uuid {
        Uuid::nil()
    }

    pub fn update_metadata(
        &mut self,
        last_generated: Option<NaiveDate>,
        last_completed: Option<NaiveDate>,
        next_scheduled: Option<NaiveDate>,
        generated_occurrences: u32,
    ) {
        self.last_generated = last_generated;
        self.last_completed = last_completed;
        self.next_scheduled = next_scheduled;
        self.generated_occurrences = generated_occurrences;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecurrenceEnd {
    Never,
    OnDate(NaiveDate),
    AfterOccurrences(u32),
}

impl Default for RecurrenceEnd {
    fn default() -> Self {
        RecurrenceEnd::Never
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecurrenceStatus {
    Active,
    Paused,
    Completed,
}

impl Default for RecurrenceStatus {
    fn default() -> Self {
        RecurrenceStatus::Active
    }
}
