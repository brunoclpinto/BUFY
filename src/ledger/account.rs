use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a financial account tracked within the ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub kind: AccountKind,
    pub category_id: Option<Uuid>,
}

impl Account {
    /// Creates a new account with the provided kind and optional linked category.
    pub fn new(name: impl Into<String>, kind: AccountKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            category_id: None,
        }
    }

    /// Links the account to a category identifier.
    pub fn with_category(mut self, category_id: Uuid) -> Self {
        self.category_id = Some(category_id);
        self
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
