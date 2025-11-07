use uuid::Uuid;

use crate::domain::account::Account;
use crate::ledger::Ledger;

use super::{ServiceError, ServiceResult};

pub struct AccountService;

impl AccountService {
    pub fn add(ledger: &mut Ledger, account: Account) -> ServiceResult<()> {
        let _ = ledger;
        let _ = account;
        Err(ServiceError::Invalid(
            "AccountService::add not yet implemented".into(),
        ))
    }

    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Account) -> ServiceResult<()> {
        let _ = ledger;
        let _ = id;
        let _ = changes;
        Err(ServiceError::Invalid(
            "AccountService::edit not yet implemented".into(),
        ))
    }

    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<()> {
        let _ = ledger;
        let _ = id;
        Err(ServiceError::Invalid(
            "AccountService::remove not yet implemented".into(),
        ))
    }

    pub fn list<'a>(ledger: &'a Ledger) -> Vec<&'a Account> {
        let _ = ledger;
        Vec::new()
    }
}
