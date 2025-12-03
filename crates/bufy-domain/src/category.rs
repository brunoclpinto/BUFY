//! Domain types representing budget categories.

use std::fmt;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::*;

/// Categorises ledger activity for budgeting and reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<Uuid>,
    pub is_custom: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<CategoryBudgetDefinition>,
}

impl Category {
    pub fn new(name: impl Into<String>, kind: CategoryKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            parent_id: None,
            is_custom: true,
            notes: None,
            budget: None,
        }
    }

    /// Returns the active budget definition, if one exists.
    pub fn budget(&self) -> Option<&CategoryBudgetDefinition> {
        self.budget.as_ref()
    }

    /// Returns `true` when the category has a budget assigned.
    pub fn has_budget(&self) -> bool {
        self.budget.is_some()
    }

    /// Assigns a budget using primitive values, overwriting prior data.
    pub fn set_budget(
        &mut self,
        amount: f64,
        period: BudgetPeriod,
        reference_date: Option<NaiveDate>,
    ) {
        self.budget = Some(CategoryBudgetDefinition {
            amount,
            period,
            reference_date,
        });
    }

    /// Assigns a pre-built budget definition.
    pub fn set_budget_definition(&mut self, definition: CategoryBudgetDefinition) {
        self.budget = Some(definition);
    }

    /// Removes any assigned budget configuration.
    pub fn clear_budget(&mut self) {
        self.budget = None;
    }
}

impl Identifiable for Category {
    fn id(&self) -> Uuid {
        self.id
    }
}

impl NamedEntity for Category {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Displayable for Category {
    fn display_label(&self) -> String {
        format!("{} ({})", self.name, self.kind)
    }
}

/// Budget settings attached directly to a category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryBudgetDefinition {
    pub amount: f64,
    pub period: BudgetPeriod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_date: Option<NaiveDate>,
}

impl CategoryBudgetDefinition {
    pub fn new(amount: f64, period: BudgetPeriod) -> Self {
        Self {
            amount,
            period,
            reference_date: None,
        }
    }

    pub fn with_reference_date(mut self, reference_date: NaiveDate) -> Self {
        self.reference_date = Some(reference_date);
        self
    }
}

/// Supported category types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CategoryKind {
    Expense,
    Income,
    Transfer,
}

impl fmt::Display for CategoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            CategoryKind::Expense => "Expense",
            CategoryKind::Income => "Income",
            CategoryKind::Transfer => "Transfer",
        };
        f.write_str(label)
    }
}
