//! Business logic helpers for managing transactions.

use uuid::Uuid;

use bufy_domain::{transaction::Transaction, Ledger};

use crate::CoreError;

/// Provides validated CRUD helpers for [`Transaction`] entities.
pub struct TransactionService;

impl TransactionService {
    /// Adds a new transaction and returns its identifier.
    pub fn add(ledger: &mut Ledger, transaction: Transaction) -> Result<Uuid, CoreError> {
        let id = ledger.add_transaction(transaction);
        Ok(id)
    }

    /// Updates the transaction identified by `id` via the provided mutator.
    pub fn update<F>(ledger: &mut Ledger, id: Uuid, mutator: F) -> Result<(), CoreError>
    where
        F: FnOnce(&mut Transaction),
    {
        let txn = ledger
            .transaction_mut(id)
            .ok_or_else(|| CoreError::TransactionNotFound(id))?;
        mutator(txn);
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        Ok(())
    }

    /// Removes the transaction identified by `id`, returning the removed instance.
    pub fn remove(ledger: &mut Ledger, id: Uuid) -> Result<Transaction, CoreError> {
        ledger
            .remove_transaction(id)
            .ok_or_else(|| CoreError::TransactionNotFound(id))
    }

    /// Returns a snapshot of the ledger's transactions.
    pub fn list(ledger: &Ledger) -> Vec<&Transaction> {
        ledger.transactions.iter().collect()
    }
}
