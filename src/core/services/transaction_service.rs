use uuid::Uuid;

use crate::domain::transaction::Transaction;
use crate::ledger::Ledger;

use super::{ServiceError, ServiceResult};

pub struct TransactionService;

impl TransactionService {
    pub fn add(ledger: &mut Ledger, transaction: Transaction) -> ServiceResult<Uuid> {
        let id = ledger.add_transaction(transaction);
        Ok(id)
    }

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

    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<Transaction> {
        ledger
            .remove_transaction(id)
            .ok_or_else(|| ServiceError::Invalid("Transaction not found".into()))
    }

    pub fn list<'a>(ledger: &'a Ledger) -> Vec<&'a Transaction> {
        ledger.transactions.iter().collect()
    }
}
