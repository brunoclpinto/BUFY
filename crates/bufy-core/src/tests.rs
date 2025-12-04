use chrono::NaiveDate;

use crate::{
    account_service::AccountService, category_service::CategoryService,
    ledger_service::LedgerService, summary_service::SummaryService,
    transaction_service::TransactionService,
};
use bufy_domain::{
    account::{Account, AccountKind},
    category::{Category, CategoryKind},
    common::{BudgetPeriod, Identifiable},
    LedgerBudgetPeriod, Transaction, TransactionStatus,
};

#[test]
fn ledger_service_creates_empty_ledger() {
    let ledger = LedgerService::create("CoreTest", LedgerBudgetPeriod::monthly());

    assert_eq!(ledger.name, "CoreTest");
    assert_eq!(ledger.budget_period, LedgerBudgetPeriod::monthly());
    assert!(ledger.accounts.is_empty());
    assert!(ledger.categories.is_empty());
    assert!(ledger.transactions.is_empty());
}

#[test]
fn account_service_adds_and_removes_accounts() {
    let mut ledger = LedgerService::create("Accounts", LedgerBudgetPeriod::monthly());
    let account = Account::new("Main", AccountKind::Bank);
    let account_id = account.id();

    AccountService::add(&mut ledger, account).expect("add account");
    assert_eq!(ledger.accounts.len(), 1);

    AccountService::remove(&mut ledger, account_id).expect("remove account");
    assert!(ledger.accounts.is_empty());
}

#[test]
fn category_service_assigns_budget() {
    let mut ledger = LedgerService::create("Categories", LedgerBudgetPeriod::monthly());
    let category = Category::new("Groceries", CategoryKind::Expense);
    let category_id = category.id();

    CategoryService::add(&mut ledger, category).expect("add category");
    CategoryService::set_budget(&mut ledger, category_id, 500.0, BudgetPeriod::Monthly, None)
        .expect("set budget");

    let stored = ledger.category(category_id).expect("category exists");
    assert!(stored.budget().is_some());
    let budget = stored.budget().unwrap();
    assert_eq!(budget.amount, 500.0);
    assert_eq!(budget.period, BudgetPeriod::Monthly);
}

#[test]
fn transaction_service_adds_and_updates_transactions() {
    let mut ledger = LedgerService::create("Transactions", LedgerBudgetPeriod::monthly());
    let account = Account::new("Checking", AccountKind::Bank);
    let account_id = account.id();
    AccountService::add(&mut ledger, account).expect("add account");

    let planned = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let transaction = Transaction::new(account_id, account_id, None, planned, 100.0);
    let txn_id = TransactionService::add(&mut ledger, transaction).expect("add transaction");
    assert_eq!(ledger.transactions.len(), 1);

    let actual = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
    TransactionService::update(&mut ledger, txn_id, |txn| txn.mark_completed(actual, 125.0))
        .expect("complete transaction");
    let stored = ledger.transaction(txn_id).expect("transaction exists");
    assert_eq!(stored.status, TransactionStatus::Completed);
    assert_eq!(stored.actual_amount, Some(125.0));
}

#[test]
fn summary_service_lists_budget_assignments() {
    let mut ledger = LedgerService::create("Summary", LedgerBudgetPeriod::monthly());
    let category = Category::new("Essentials", CategoryKind::Expense);
    let category_id = category.id();
    CategoryService::add(&mut ledger, category).expect("add category");
    CategoryService::set_budget(&mut ledger, category_id, 250.0, BudgetPeriod::Monthly, None)
        .expect("set budget");

    let assignments = SummaryService::categories_with_budgets(&ledger);
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].category_id, category_id);
}
