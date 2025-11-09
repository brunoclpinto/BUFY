use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::common::*;

/// Categorises ledger activity for budgeting and reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<Uuid>,
    pub is_custom: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
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
        }
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
        format!("{} ({:?})", self.name, self.kind)
    }
}

/// Supported category types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CategoryKind {
    Expense,
    Income,
    Transfer,
}
