use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::time_interval::TimeInterval;

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
        if next_scheduled.is_none() && !matches!(self.status, RecurrenceStatus::Paused) {
            self.status = RecurrenceStatus::Completed;
        } else if next_scheduled.is_some() && !matches!(self.status, RecurrenceStatus::Paused) {
            self.status = RecurrenceStatus::Active;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RecurrenceEnd {
    #[default]
    Never,
    OnDate(NaiveDate),
    AfterOccurrences(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RecurrenceStatus {
    #[default]
    Active,
    Paused,
    Completed,
}
