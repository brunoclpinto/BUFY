//! Simulation domain types used by CLI and services.

use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ledger::{transaction::Transaction, BudgetSummary, BudgetTotalsDelta};

fn default_simulation_id() -> Uuid {
    Uuid::new_v4()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Captures the before/after comparison for a simulation run.
pub struct SimulationBudgetImpact {
    pub simulation_name: String,
    pub base: BudgetSummary,
    pub simulated: BudgetSummary,
    pub delta: BudgetTotalsDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents a named what-if scenario comprised of change sets.
pub struct Simulation {
    #[serde(default = "default_simulation_id")]
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub status: SimulationStatus,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub applied_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub changes: Vec<SimulationChange>,
}

impl Simulation {
    /// Creates a new simulation with the provided display name.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            notes: None,
            status: SimulationStatus::Pending,
            created_at: now,
            updated_at: now,
            applied_at: None,
            changes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Enumerates the lifecycle state of a simulation.
#[derive(Default)]
pub enum SimulationStatus {
    #[default]
    Pending,
    Applied,
    Discarded,
}

impl fmt::Display for SimulationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            SimulationStatus::Pending => "Pending",
            SimulationStatus::Applied => "Applied",
            SimulationStatus::Discarded => "Discarded",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
/// Tracks an individual change within a simulation.
pub enum SimulationChange {
    AddTransaction { transaction: Transaction },
    ModifyTransaction(SimulationTransactionPatch),
    ExcludeTransaction { transaction_id: Uuid },
}

impl SimulationChange {
    pub fn summary(&self) -> String {
        match self {
            SimulationChange::AddTransaction { transaction } => {
                format!("Add transaction {}", transaction.id)
            }
            SimulationChange::ModifyTransaction(patch) => {
                format!("Modify transaction {}", patch.transaction_id)
            }
            SimulationChange::ExcludeTransaction { transaction_id } => {
                format!("Remove transaction {}", transaction_id)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents a mutation to an existing or simulated transaction.
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
    /// Determines whether the patch mutates at least one attribute.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Simplified change log entry used after simulation evaluation.
pub struct SimulatedChange {
    pub target_id: Uuid,
    pub change_type: ChangeKind,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Describes the type of simulated mutation.
pub enum ChangeKind {
    Add,
    Modify,
    Remove,
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            ChangeKind::Add => "Add",
            ChangeKind::Modify => "Modify",
            ChangeKind::Remove => "Remove",
        };
        f.write_str(label)
    }
}

impl From<&SimulationChange> for SimulatedChange {
    fn from(change: &SimulationChange) -> Self {
        match change {
            SimulationChange::AddTransaction { transaction } => Self {
                target_id: transaction.id,
                change_type: ChangeKind::Add,
                delta: transaction
                    .actual_amount
                    .or(Some(transaction.budgeted_amount))
                    .unwrap_or(0.0),
            },
            SimulationChange::ModifyTransaction(patch) => Self {
                target_id: patch.transaction_id,
                change_type: ChangeKind::Modify,
                delta: patch
                    .actual_amount
                    .flatten()
                    .or(patch.budgeted_amount)
                    .unwrap_or(0.0),
            },
            SimulationChange::ExcludeTransaction { transaction_id } => Self {
                target_id: *transaction_id,
                change_type: ChangeKind::Remove,
                delta: 0.0,
            },
        }
    }
}
