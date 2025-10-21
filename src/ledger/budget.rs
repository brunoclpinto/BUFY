use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A spending guardrail for a specific category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub id: Uuid,
    pub category_id: Uuid,
    pub limit_cents: i64,
    pub period: BudgetPeriod,
}

impl Budget {
    pub fn new(category_id: Uuid, limit_cents: i64, period: BudgetPeriod) -> Self {
        Self {
            id: Uuid::new_v4(),
            category_id,
            limit_cents,
            period,
        }
    }
}

/// Enumeration of budgeting periods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetPeriod {
    Monthly,
    Quarterly,
    Yearly,
}
