//! Business logic helpers for managing transactions.

use uuid::Uuid;

use crate::core::services::{ServiceError, ServiceResult};
use crate::domain::transaction::Transaction;
use crate::ledger::Ledger;

/// Provides validated CRUD helpers for ledger transactions.
pub struct TransactionService;

impl TransactionService {
    /// Adds a new transaction and returns its identifier.
    pub fn add(ledger: &mut Ledger, transaction: Transaction) -> ServiceResult<Uuid> {
        let id = ledger.add_transaction(transaction);
        Ok(id)
    }

    /// Updates the transaction identified by `id` via the provided mutator.
    pub fn update<F>(ledger: &mut Ledger, id: Uuid, mutator: F) -> ServiceResult<()>
    where
        F: FnOnce(&mut Transaction),
    {
        let txn = ledger
            .transaction_mut(id)
            .ok_or_else(|| ServiceError::Invalid("Transaction not found".into()))?;
        mutator(txn);
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        Ok(())
    }

    /// Removes the transaction identified by `id`, returning the removed instance.
    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<Transaction> {
        ledger
            .remove_transaction(id)
            .ok_or_else(|| ServiceError::Invalid("Transaction not found".into()))
    }

    /// Returns a snapshot of the ledger's transactions.
    pub fn list(ledger: &Ledger) -> Vec<&Transaction> {
        ledger.transactions.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::transaction::Transaction;
    use crate::ledger::{BudgetPeriod, Ledger};
    use chrono::NaiveDate;

    fn base_ledger() -> Ledger {
        Ledger::new("Txn", BudgetPeriod::monthly())
    }

    fn sample_transaction() -> Transaction {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        Transaction::new(Uuid::new_v4(), Uuid::new_v4(), None, date, 42.0)
    }

    #[test]
    fn update_fails_for_missing_transaction() {
        let mut ledger = base_ledger();
        let err = TransactionService::update(&mut ledger, Uuid::new_v4(), |_| {})
            .expect_err("update must fail for unknown id");
        assert!(
            matches!(err, ServiceError::Invalid(ref message) if message.contains("not found")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn remove_returns_deleted_transaction() {
        let mut ledger = base_ledger();
        let txn = sample_transaction();
        let txn_id = txn.id;
        TransactionService::add(&mut ledger, txn).unwrap();

        let removed = TransactionService::remove(&mut ledger, txn_id).unwrap();
        assert_eq!(removed.id, txn_id);
        assert!(ledger.transaction(txn_id).is_none());
    }
}
