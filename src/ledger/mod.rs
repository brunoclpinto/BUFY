//! Ledger domain models and management utilities.

pub mod account;
pub mod budget;
pub mod category;
pub mod transaction;

pub use account::Account;
pub use budget::{Budget, BudgetPeriod};
pub use category::Category;
pub use transaction::Transaction;

use crate::errors::LedgerError;
use std::collections::HashMap;
use uuid::Uuid;

/// In-memory ledger state used for simulations and bookkeeping.
#[derive(Default)]
pub struct Ledger {
    accounts: HashMap<Uuid, Account>,
    categories: HashMap<Uuid, Category>,
    budgets: HashMap<Uuid, Budget>,
    transactions: Vec<Transaction>,
}

impl Ledger {
    /// Creates a new empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an account with the ledger.
    pub fn insert_account(&mut self, account: Account) -> Uuid {
        let id = account.id;
        self.accounts.insert(id, account);
        id
    }

    /// Fetches an account, returning a descriptive error if unknown.
    pub fn account(&self, id: Uuid) -> Result<&Account, LedgerError> {
        self.accounts
            .get(&id)
            .ok_or_else(|| LedgerError::InvalidRef(format!("account {id} not found")))
    }

    /// Registers a category.
    pub fn insert_category(&mut self, category: Category) -> Uuid {
        let id = category.id;
        self.categories.insert(id, category);
        id
    }

    /// Adds a budget definition.
    pub fn insert_budget(&mut self, budget: Budget) -> Uuid {
        let id = budget.id;
        self.budgets.insert(id, budget);
        id
    }

    /// Records a transaction.
    pub fn record_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }

    /// Returns the number of transactions recorded.
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }
}
