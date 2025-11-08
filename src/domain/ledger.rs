use std::cmp::Ordering;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    core::errors::BudgetError,
    currency::{policy_date, ValuationPolicy},
    domain::transaction::{TimeInterval, Transaction},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DateWindow {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateWindow {
    pub fn new(start: NaiveDate, end: NaiveDate) -> Result<Self, BudgetError> {
        if end <= start {
            return Err(BudgetError::InvalidInput(
                "window end must be after start".into(),
            ));
        }
        Ok(Self { start, end })
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date < self.end
    }

    pub fn shift(&self, interval: &TimeInterval, steps: i32) -> Self {
        let new_start = interval.add_to(self.start, steps);
        let new_end = interval.add_to(self.end, steps);
        Self {
            start: new_start,
            end: new_end,
        }
    }

    pub fn scope(&self, reference: NaiveDate) -> BudgetScope {
        if self.contains(reference) {
            BudgetScope::Current
        } else if self.end <= reference {
            BudgetScope::Past
        } else if self.start > reference {
            BudgetScope::Future
        } else {
            BudgetScope::Custom
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetScope {
    Past,
    Current,
    Future,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetTotals {
    pub budgeted: f64,
    pub real: f64,
    pub remaining: f64,
    pub variance: f64,
    pub percent_used: Option<f64>,
    pub status: BudgetStatus,
    pub incomplete: bool,
}

impl BudgetTotals {
    pub fn from_parts(budgeted: f64, real: f64, incomplete: bool) -> Self {
        let remaining = budgeted - real;
        let variance = real - budgeted;
        let percent_used = if budgeted.abs() > f64::EPSILON {
            Some((real / budgeted) * 100.0)
        } else if real.abs() > f64::EPSILON {
            Some(100.0)
        } else {
            None
        };
        let status = if incomplete {
            BudgetStatus::Incomplete
        } else if budgeted.abs() < f64::EPSILON && real.abs() < f64::EPSILON {
            BudgetStatus::Empty
        } else {
            match real.partial_cmp(&budgeted).unwrap_or(Ordering::Equal) {
                Ordering::Greater => BudgetStatus::OverBudget,
                Ordering::Less => BudgetStatus::UnderBudget,
                Ordering::Equal => BudgetStatus::OnTrack,
            }
        };
        Self {
            budgeted,
            real,
            remaining,
            variance,
            percent_used,
            status,
            incomplete,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetStatus {
    OnTrack,
    OverBudget,
    UnderBudget,
    Empty,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBudget {
    pub category_id: Option<Uuid>,
    pub name: String,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBudget {
    pub account_id: Uuid,
    pub name: String,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummary {
    pub scope: BudgetScope,
    pub window: DateWindow,
    pub totals: BudgetTotals,
    pub per_category: Vec<CategoryBudget>,
    pub per_account: Vec<AccountBudget>,
    pub orphaned_transactions: usize,
    pub incomplete_transactions: usize,
    #[serde(default)]
    pub disclosures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetTotalsDelta {
    pub budgeted: f64,
    pub real: f64,
    pub remaining: f64,
    pub variance: f64,
}

#[derive(Debug, Clone)]
pub struct ConversionContext {
    pub policy: ValuationPolicy,
    pub report_date: NaiveDate,
}

impl ConversionContext {
    pub fn effective_date(&self, txn_date: NaiveDate) -> NaiveDate {
        policy_date(&self.policy, txn_date, self.report_date)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationBudgetImpact {
    pub simulation_name: String,
    pub base: BudgetSummary,
    pub simulated: BudgetSummary,
    pub delta: BudgetTotalsDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Simulation {
    pub name: String,
    pub notes: Option<String>,
    pub status: SimulationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub applied_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub changes: Vec<SimulationChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SimulationStatus {
    Pending,
    Applied,
    Discarded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SimulationChange {
    AddTransaction { transaction: Transaction },
    ModifyTransaction(SimulationTransactionPatch),
    ExcludeTransaction { transaction_id: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationTransactionPatch {
    pub transaction_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_account: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_account: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<Option<Uuid>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_date: Option<Option<NaiveDate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budgeted_amount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_amount: Option<Option<f64>>,
}

impl SimulationTransactionPatch {
    pub fn has_effect(&self) -> bool {
        self.from_account.is_some()
            || self.to_account.is_some()
            || self.category_id.is_some()
            || self.scheduled_date.is_some()
            || self.actual_date.is_some()
            || self.budgeted_amount.is_some()
            || self.actual_amount.is_some()
    }
}
