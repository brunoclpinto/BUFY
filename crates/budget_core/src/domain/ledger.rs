//! Ledger-level budgeting structures and reporting helpers.

use std::{cmp::Ordering, fmt};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::common::*;
use crate::{
    core::errors::BudgetError,
    currency::{policy_date, ValuationPolicy},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
/// Defines a reporting window for budget summaries.
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
/// Identifies how a date window maps to the active budgeting period.
pub enum BudgetScope {
    Past,
    Current,
    Future,
    Custom,
}

impl fmt::Display for BudgetScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            BudgetScope::Past => "Past",
            BudgetScope::Current => "Current",
            BudgetScope::Future => "Future",
            BudgetScope::Custom => "Custom",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Aggregated totals for a single budgeting bucket.
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
/// Describes whether the budget is aligned with the plan.
pub enum BudgetStatus {
    OnTrack,
    OverBudget,
    UnderBudget,
    Empty,
    Incomplete,
}

impl fmt::Display for BudgetStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            BudgetStatus::OnTrack => "On Track",
            BudgetStatus::OverBudget => "Over Budget",
            BudgetStatus::UnderBudget => "Under Budget",
            BudgetStatus::Empty => "Empty",
            BudgetStatus::Incomplete => "Incomplete",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Aggregated totals for a single category.
pub struct CategoryBudget {
    pub category_id: Option<Uuid>,
    pub name: String,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Aggregated totals tied to an account.
pub struct AccountBudget {
    pub account_id: Uuid,
    pub name: String,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Complete summary for a selected window, including per-category/account totals.
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
/// Differences between baseline and simulated totals.
pub struct BudgetTotalsDelta {
    pub budgeted: f64,
    pub real: f64,
    pub remaining: f64,
    pub variance: f64,
}

#[derive(Debug, Clone)]
/// Supplies valuation context for currency conversions.
pub struct ConversionContext {
    pub policy: ValuationPolicy,
    pub report_date: NaiveDate,
}

impl ConversionContext {
    pub fn effective_date(&self, txn_date: NaiveDate) -> NaiveDate {
        policy_date(&self.policy, txn_date, self.report_date)
    }
}
