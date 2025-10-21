use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a budgeting category for grouping transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
}

impl Category {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
        }
    }
}
