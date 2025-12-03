//! Business logic helpers for validated account mutations.

use uuid::Uuid;

use bufy_domain::{account::Account, Ledger};

use crate::CoreError;

/// Provides validated mutations for [`Account`] entities.
///
/// See also [`crate::category_service::CategoryService`] when linking accounts to categories.
pub struct AccountService;

impl AccountService {
    /// Adds a new account after validating uniqueness and linked category.
    pub fn add(ledger: &mut Ledger, account: Account) -> Result<(), CoreError> {
        Self::validate_name(ledger, None, &account.name)?;
        if let Some(category_id) = account.category_id {
            Self::ensure_category_exists(ledger, category_id)?;
        }
        ledger.add_account(account);
        Ok(())
    }

    /// Updates an existing account by applying the provided changeset.
    pub fn edit(ledger: &mut Ledger, id: Uuid, changes: Account) -> Result<(), CoreError> {
        Self::validate_name(ledger, Some(id), &changes.name)?;
        if let Some(category_id) = changes.category_id {
            Self::ensure_category_exists(ledger, category_id)?;
        }
        let account = ledger
            .account_mut(id)
            .ok_or_else(|| CoreError::AccountNotFound(id.to_string()))?;
        account.name = changes.name;
        account.kind = changes.kind;
        account.category_id = changes.category_id;
        account.opening_balance = changes.opening_balance;
        account.notes = changes.notes;
        account.currency = changes.currency;
        ledger.touch();
        Ok(())
    }

    /// Removes an account when no linked transactions exist.
    pub fn remove(ledger: &mut Ledger, id: Uuid) -> Result<(), CoreError> {
        if ledger
            .transactions
            .iter()
            .any(|txn| txn.from_account == id || txn.to_account == id)
        {
            return Err(CoreError::InvalidOperation(
                "account has linked transactions".into(),
            ));
        }
        let before = ledger.accounts.len();
        ledger.accounts.retain(|account| account.id != id);
        if ledger.accounts.len() == before {
            return Err(CoreError::AccountNotFound(id.to_string()));
        }
        ledger.touch();
        Ok(())
    }

    /// Returns a snapshot of the accounts currently tracked in the ledger.
    pub fn list(ledger: &Ledger) -> Vec<&Account> {
        ledger.accounts.iter().collect()
    }

    fn validate_name(
        ledger: &Ledger,
        exclude: Option<Uuid>,
        candidate: &str,
    ) -> Result<(), CoreError> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let duplicate = ledger.accounts.iter().any(|account| {
            let name = account.name.trim().to_ascii_lowercase();
            name == normalized && (exclude != Some(account.id))
        });
        if duplicate {
            Err(CoreError::Validation(format!(
                "account `{}` already exists",
                candidate
            )))
        } else {
            Ok(())
        }
    }

    fn ensure_category_exists(ledger: &Ledger, category_id: Uuid) -> Result<(), CoreError> {
        if ledger.category(category_id).is_some() {
            Ok(())
        } else {
            Err(CoreError::CategoryNotFound(category_id.to_string()))
        }
    }
}
