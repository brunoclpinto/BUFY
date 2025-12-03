//! Ledger-level budgeting structures and reporting helpers.

use std::{cmp::Ordering, fmt};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{category::CategoryBudgetDefinition, common::*};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
/// Defines a reporting window for budget summaries.
pub struct DateWindow {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateWindow {
    pub fn new(start: NaiveDate, end: NaiveDate) -> Result<Self, DateWindowError> {
        if end <= start {
            return Err(DateWindowError::InvalidRange);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Errors that can occur when constructing [`DateWindow`] values.
pub enum DateWindowError {
    InvalidRange,
}

impl fmt::Display for DateWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateWindowError::InvalidRange => {
                f.write_str("date window end must be after start")
            }
        }
    }
}

impl std::error::Error for DateWindowError {}

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

/// Mirrors the budgeting cadence used for a category budget definition.
pub type CategoryBudgetPeriod = BudgetPeriod;

/// Snapshot describing a category with an explicit budget definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryBudgetAssignment {
    pub category_id: Uuid,
    pub name: String,
    pub budget: CategoryBudgetDefinition,
}

/// Combines spending totals with the category's configured budget.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryBudgetStatus {
    pub category_id: Uuid,
    pub name: String,
    pub budget: Option<CategoryBudgetDefinition>,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CategoryBudgetSummaryKind {
    Actual,
    Projected,
    Simulated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryBudgetSummary {
    pub category_id: Uuid,
    pub name: String,
    pub budget_amount: f64,
    pub spent_amount: f64,
    pub remaining_amount: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utilization_percent: Option<f64>,
    pub status: BudgetStatus,
    pub period: CategoryBudgetPeriod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_date: Option<NaiveDate>,
    pub kind: CategoryBudgetSummaryKind,
}

impl CategoryBudgetSummary {
    pub fn from_definition(
        category_id: Uuid,
        name: String,
        budget: &CategoryBudgetDefinition,
        spent: f64,
        kind: CategoryBudgetSummaryKind,
    ) -> Self {
        let totals = BudgetTotals::from_parts(budget.amount, spent, false);
        Self {
            category_id,
            name,
            budget_amount: budget.amount,
            spent_amount: spent,
            remaining_amount: budget.amount - spent,
            utilization_percent: totals.percent_used,
            status: totals.status,
            period: budget.period.clone(),
            reference_date: budget.reference_date,
            kind,
        }
    }
}
