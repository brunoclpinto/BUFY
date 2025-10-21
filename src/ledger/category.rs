use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Categorises ledger activity for budgeting and reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<Uuid>,
    pub is_custom: bool,
}

impl Category {
    pub fn new(name: impl Into<String>, kind: CategoryKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            parent_id: None,
            is_custom: true,
        }
    }
}

/// Supported category types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CategoryKind {
    Expense,
    Income,
    Transfer,
}
