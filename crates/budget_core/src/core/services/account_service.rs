//! Business logic helpers for validated account mutations.

use uuid::Uuid;

use crate::core::services::{ServiceError, ServiceResult};
use bufy_domain::account::Account;
use crate::ledger::Ledger;

/// Provides validated mutations for [`Account`] entities.
///
/// See also: [`crate::core::services::CategoryService`] for linked category validation.
pub struct AccountService;

impl AccountService {
    /// Adds a new account after validating uniqueness and linked category.
    pub fn add(ledger: &mut Ledger, account: Account) -> ServiceResult<()> {
        Self::validate_name(ledger, None, &account.name)?;
        if let Some(category_id) = account.category_id {
            Self::ensure_category_exists(ledger, category_id)?;
        }
        ledger.add_account(account);
        Ok(())
    }

    /// Updates an existing account by applying the provided changeset.
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

    /// Removes an account when no linked transactions exist.
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

    /// Returns a snapshot of the accounts currently tracked in the ledger.
    pub fn list(ledger: &Ledger) -> Vec<&Account> {
        ledger.accounts.iter().collect()
    }

    fn validate_name(ledger: &Ledger, exclude: Option<Uuid>, candidate: &str) -> ServiceResult<()> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let duplicate = ledger.accounts.iter().any(|account| {
            let name = account.name.trim().to_ascii_lowercase();
            name == normalized && (exclude != Some(account.id))
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

#[cfg(test)]
mod tests {
    use super::*;
    use bufy_domain::{
        account::{Account, AccountKind},
        category::{Category, CategoryKind},
    };
    use crate::ledger::{BudgetPeriod, Ledger};

    fn ledger_with_category() -> (Ledger, Uuid) {
        let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
        let category = Category::new("Utilities", CategoryKind::Expense);
        let category_id = category.id;
        ledger.add_category(category);
        (ledger, category_id)
    }

    #[test]
    fn add_rejects_duplicate_names() {
        let (mut ledger, _) = ledger_with_category();
        let primary = Account::new("Checking", AccountKind::Bank);
        AccountService::add(&mut ledger, primary.clone()).expect("first add succeeds");

        let err = AccountService::add(&mut ledger, primary).expect_err("duplicate must fail");
        assert!(
            matches!(err, ServiceError::Invalid(ref message) if message.contains("already exists")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn edit_overwrites_account_fields() {
        let (mut ledger, category_id) = ledger_with_category();
        let mut account = Account::new("Checking", AccountKind::Bank);
        account.category_id = Some(category_id);
        AccountService::add(&mut ledger, account.clone()).expect("add succeeds");

        let mut changes = Account::new("Updated", AccountKind::Savings);
        changes.category_id = None;
        changes.opening_balance = Some(25.0);
        changes.notes = Some("Notes".into());
        AccountService::edit(&mut ledger, account.id, changes).expect("edit succeeds");

        let stored = ledger.account(account.id).expect("account exists");
        assert_eq!(stored.name, "Updated");
        assert_eq!(stored.kind, AccountKind::Savings);
        assert_eq!(stored.opening_balance, Some(25.0));
        assert!(stored.category_id.is_none());
        assert_eq!(stored.notes.as_deref(), Some("Notes"));
    }
}
