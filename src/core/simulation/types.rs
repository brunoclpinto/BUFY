use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ledger::{transaction::Transaction, BudgetSummary, BudgetTotalsDelta};

fn default_simulation_id() -> Uuid {
    Uuid::new_v4()
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
pub enum SimulationStatus {
    Pending,
    Applied,
    Discarded,
}

impl Default for SimulationStatus {
    fn default() -> Self {
        SimulationStatus::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedChange {
    pub target_id: Uuid,
    pub change_type: ChangeKind,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeKind {
    Add,
    Modify,
    Remove,
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
