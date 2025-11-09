use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::common::*;

/// Represents a financial account tracked within the ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub kind: AccountKind,
    pub category_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening_balance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl Account {
    /// Creates a new account with the provided kind and optional linked category.
    pub fn new(name: impl Into<String>, kind: AccountKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            category_id: None,
            currency: None,
            opening_balance: None,
            notes: None,
        }
    }

    /// Links the account to a category identifier.
    pub fn with_category(mut self, category_id: Uuid) -> Self {
        self.category_id = Some(category_id);
        self
    }
}

impl Identifiable for Account {
    fn id(&self) -> Uuid {
        self.id
    }
}

impl NamedEntity for Account {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Displayable for Account {
    fn display_label(&self) -> String {
        format!("{} ({:?})", self.name, self.kind)
    }
}

/// Enumerates the supported account classifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountKind {
    Bank,
    Cash,
    Savings,
    ExpenseDestination,
    IncomeSource,
    Unknown,
}
