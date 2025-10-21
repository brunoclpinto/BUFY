use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a financial account that can contain multiple transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub balance_cents: i64,
}

impl Account {
    /// Creates a new account with a zero balance.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            balance_cents: 0,
        }
    }
}
