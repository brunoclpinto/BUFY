use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    account::Account,
    category::Category,
    time_interval::{TimeInterval, TimeUnit},
    transaction::Transaction,
};

const CURRENT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub id: Uuid,
    pub name: String,
    pub budget_period: BudgetPeriod,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default = "Ledger::schema_version_default")]
    pub schema_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetPeriod(pub TimeInterval);

impl BudgetPeriod {
    pub fn monthly() -> Self {
        Self(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        })
    }
}

impl Default for BudgetPeriod {
    fn default() -> Self {
        Self::monthly()
    }
}

impl Ledger {
    pub fn new(name: impl Into<String>, budget_period: BudgetPeriod) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            budget_period,
            accounts: Vec::new(),
            categories: Vec::new(),
            transactions: Vec::new(),
            created_at: now,
            updated_at: now,
            schema_version: CURRENT_SCHEMA_VERSION,
        }
    }

    pub fn add_account(&mut self, account: Account) -> Uuid {
        let id = account.id;
        self.accounts.push(account);
        self.touch();
        id
    }

    pub fn add_category(&mut self, category: Category) -> Uuid {
        let id = category.id;
        self.categories.push(category);
        self.touch();
        id
    }

    pub fn add_transaction(&mut self, transaction: Transaction) -> Uuid {
        let id = transaction.id;
        self.transactions.push(transaction);
        self.touch();
        id
    }

    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    pub fn account(&self, id: Uuid) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn schema_version_default() -> u8 {
        CURRENT_SCHEMA_VERSION
    }
}
