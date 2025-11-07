use uuid::Uuid;

use crate::domain::transaction::Transaction;
use crate::ledger::Ledger;

use super::{ServiceError, ServiceResult};

pub struct TransactionService;

impl TransactionService {
    pub fn add(ledger: &mut Ledger, transaction: Transaction) -> ServiceResult<()> {
        let _ = ledger;
        let _ = transaction;
        Err(ServiceError::Invalid(
            "TransactionService::add not yet implemented".into(),
        ))
    }

    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Transaction) -> ServiceResult<()> {
        let _ = ledger;
        let _ = id;
        let _ = changes;
        Err(ServiceError::Invalid(
            "TransactionService::edit not yet implemented".into(),
        ))
    }

    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<()> {
        let _ = ledger;
        let _ = id;
        Err(ServiceError::Invalid(
            "TransactionService::remove not yet implemented".into(),
        ))
    }

    pub fn list<'a>(ledger: &'a Ledger) -> Vec<&'a Transaction> {
        let _ = ledger;
        Vec::new()
    }
}
