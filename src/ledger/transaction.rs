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
    pub recurrence: Option<Recurrence>,
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
            recurrence: None,
            status: TransactionStatus::Planned,
        }
    }

    pub fn with_recurrence(mut self, recurrence: Recurrence) -> Self {
        self.recurrence = Some(recurrence);
        self
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
    pub interval: TimeInterval,
    pub mode: RecurrenceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecurrenceMode {
    /// Follows fixed planned schedule regardless of actual timing.
    FixedSchedule,
    /// Starts next period after the actual performed date.
    AfterLastPerformed,
}

impl Recurrence {
    pub fn next_occurrence(
        &self,
        last_scheduled: NaiveDate,
        last_performed: Option<NaiveDate>,
    ) -> NaiveDate {
        match self.mode {
            RecurrenceMode::FixedSchedule => self.interval.next_date(last_scheduled),
            RecurrenceMode::AfterLastPerformed => {
                if let Some(performed) = last_performed {
                    self.interval.next_date(performed)
                } else {
                    self.interval.next_date(last_scheduled)
                }
            }
        }
    }
}
