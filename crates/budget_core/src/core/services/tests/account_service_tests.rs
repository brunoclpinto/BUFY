use crate::core::services::{AccountService, ServiceError};
use crate::domain::account::{Account, AccountKind};
use crate::ledger::{BudgetPeriod, Ledger};

#[test]
fn add_account_increments_count() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let account = Account::new("Checking", AccountKind::Bank);

    AccountService::add(&mut ledger, account).unwrap();

    assert_eq!(ledger.accounts.len(), 1);
}

#[test]
fn duplicate_name_is_rejected() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    AccountService::add(&mut ledger, Account::new("Wallet", AccountKind::Cash)).unwrap();

    let err =
        AccountService::add(&mut ledger, Account::new("wallet", AccountKind::Cash)).unwrap_err();
    assert!(matches!(err, ServiceError::Invalid(_)));
}

#[test]
fn edit_updates_fields() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let account = Account::new("Primary", AccountKind::Bank);
    let id = account.id;
    AccountService::add(&mut ledger, account).unwrap();

    let mut changes = Account::new("Primary 2", AccountKind::Savings);
    changes.id = id;
    AccountService::edit(&mut ledger, id, changes).unwrap();

    let updated = ledger.account(id).unwrap();
    assert_eq!(updated.name, "Primary 2");
    assert_eq!(updated.kind, AccountKind::Savings);
}
