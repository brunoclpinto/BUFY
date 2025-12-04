//! bufy-domain
//!
//! Pure domain models (Ledger, Account, Category, Transaction, Simulation, etc.).
//! No I/O, no CLI, no storage. Only data types and core enums.

pub mod account;
pub mod category;
pub mod common;
pub mod currency;
pub mod ledger;
pub mod ledger_data;
pub mod recurring;
pub mod simulation;
pub mod transaction;

pub use account::*;
pub use category::*;
pub use common::*;
pub use currency::*;
pub use ledger::*;
pub use ledger_data::*;
pub use recurring::*;
pub use simulation::*;
pub use transaction::*;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn ledger_can_hold_accounts_categories_and_transactions() {
        let mut ledger = Ledger::new("TestLedger", LedgerBudgetPeriod::monthly());

        let account = Account::new("Main Account", AccountKind::Bank);
        let account_id = account.id;
        ledger.accounts.push(account);

        let category = Category::new("Groceries", CategoryKind::Expense);
        let category_id = category.id;
        ledger.categories.push(category);

        let mut transaction = Transaction::new(
            account_id,
            account_id,
            Some(category_id),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            100.0,
        );
        transaction.notes = Some("Initial grocery budget".to_string());
        ledger.transactions.push(transaction);

        assert_eq!(ledger.accounts.len(), 1);
        assert_eq!(ledger.categories.len(), 1);
        assert_eq!(ledger.transactions.len(), 1);
    }

    #[test]
    fn ledger_roundtrips_through_serde_json() {
        let ledger = Ledger::new("RoundTrip", LedgerBudgetPeriod::monthly());

        let json = serde_json::to_string(&ledger).expect("serialize ledger");
        let decoded: Ledger = serde_json::from_str(&json).expect("deserialize ledger");

        assert_eq!(decoded.name, "RoundTrip");
        assert_eq!(decoded.budget_period, ledger.budget_period);
    }
}
