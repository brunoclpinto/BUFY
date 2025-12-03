use crate::core::services::TransactionService;
use bufy_domain::{
    account::{Account, AccountKind},
    transaction::{Transaction, TransactionStatus},
};
use crate::ledger::{BudgetPeriod, Ledger};

#[test]
fn add_transaction_returns_id() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let from = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let txn = Transaction::new(
        from,
        to,
        None,
        chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        100.0,
    );

    let id = TransactionService::add(&mut ledger, txn).unwrap();

    assert!(ledger.transaction(id).is_some());
}

#[test]
fn update_transaction_mutates_struct() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let from = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let txn = Transaction::new(
        from,
        to,
        None,
        chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        100.0,
    );
    let id = TransactionService::add(&mut ledger, txn).unwrap();

    TransactionService::update(&mut ledger, id, |t| {
        t.status = TransactionStatus::Completed;
    })
    .unwrap();

    assert!(matches!(
        ledger.transaction(id).unwrap().status,
        TransactionStatus::Completed
    ));
}

#[test]
fn remove_transaction_deletes_entry() {
    let mut ledger = Ledger::new("Test", BudgetPeriod::monthly());
    let from = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let txn = Transaction::new(
        from,
        to,
        None,
        chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        100.0,
    );
    let id = TransactionService::add(&mut ledger, txn).unwrap();

    TransactionService::remove(&mut ledger, id).unwrap();

    assert!(ledger.transaction(id).is_none());
}
