use uuid::Uuid;

use crate::domain::account::Account;
use crate::ledger::Ledger;

use super::{ServiceError, ServiceResult};

pub struct AccountService;

impl AccountService {
    pub fn add(ledger: &mut Ledger, account: Account) -> ServiceResult<()> {
        Self::validate_name(ledger, None, &account.name)?;
        if let Some(category_id) = account.category_id {
            Self::ensure_category_exists(ledger, category_id)?;
        }
        ledger.add_account(account);
        Ok(())
    }

    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Account) -> ServiceResult<()> {
        Self::validate_name(ledger, Some(id), &changes.name)?;
        if let Some(category_id) = changes.category_id {
            Self::ensure_category_exists(ledger, category_id)?;
        }
        let account = ledger
            .account_mut(id)
            .ok_or_else(|| ServiceError::Invalid("Account not found".into()))?;
        account.name = changes.name;
        account.kind = changes.kind;
        account.category_id = changes.category_id;
        account.opening_balance = changes.opening_balance;
        account.notes = changes.notes;
        account.currency = changes.currency;
        ledger.touch();
        Ok(())
    }

    pub fn remove(ledger: &mut Ledger, id: Uuid) -> ServiceResult<()> {
        if ledger
            .transactions
            .iter()
            .any(|txn| txn.from_account == id || txn.to_account == id)
        {
            return Err(ServiceError::Invalid(
                "Account has linked transactions".into(),
            ));
        }
        let before = ledger.accounts.len();
        ledger.accounts.retain(|account| account.id != id);
        if ledger.accounts.len() == before {
            return Err(ServiceError::Invalid("Account not found".into()));
        }
        ledger.touch();
        Ok(())
    }

    pub fn list<'a>(ledger: &'a Ledger) -> Vec<&'a Account> {
        ledger.accounts.iter().collect()
    }

    fn validate_name(ledger: &Ledger, exclude: Option<Uuid>, candidate: &str) -> ServiceResult<()> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let duplicate = ledger.accounts.iter().any(|account| {
            let name = account.name.trim().to_ascii_lowercase();
            name == normalized && exclude.map_or(true, |id| account.id != id)
        });
        if duplicate {
            Err(ServiceError::Invalid(format!(
                "Account `{}` already exists",
                candidate
            )))
        } else {
            Ok(())
        }
    }

    fn ensure_category_exists(ledger: &Ledger, category_id: Uuid) -> ServiceResult<()> {
        if ledger.category(category_id).is_some() {
            Ok(())
        } else {
            Err(ServiceError::Invalid(
                "Linked category does not exist".into(),
            ))
        }
    }
}
