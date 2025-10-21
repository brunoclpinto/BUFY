use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a financial movement against an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub account_id: Uuid,
    pub category_id: Option<Uuid>,
    pub amount_cents: i64,
    pub description: String,
    pub timestamp: DateTime<Utc>,
}

impl Transaction {
    /// Creates a new transaction occurring at the current time.
    pub fn new(
        account_id: Uuid,
        category_id: Option<Uuid>,
        amount_cents: i64,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            account_id,
            category_id,
            amount_cents,
            description: description.into(),
            timestamp: Utc::now(),
        }
    }
}
