use budget_core::{
    core::services::{AccountService, CategoryService, SummaryService, TransactionService},
    domain::{
        account::{Account, AccountKind},
        category::{Category, CategoryKind},
        ledger::{BudgetScope, DateWindow},
    },
    ledger::{BudgetPeriod, Ledger, Transaction},
};
use chrono::NaiveDate;

fn prepared_ledger() -> Ledger {
    let mut ledger = Ledger::new("Services", BudgetPeriod::monthly());
    let income = Category::new("Income", CategoryKind::Income);
    let expense = Category::new("Bills", CategoryKind::Expense);
    let expense_id = expense.id;
    ledger.add_category(income);
    ledger.add_category(expense);
    let checking = Account::new("Checking", AccountKind::Bank);
    AccountService::add(&mut ledger, checking.clone()).unwrap();
    let savings = Account::new("Savings", AccountKind::Savings);
    AccountService::add(&mut ledger, savings.clone()).unwrap();

    let date = NaiveDate::from_ymd_opt(2024, 2, 10).unwrap();
    let mut txn = Transaction::new(checking.id, savings.id, Some(expense_id), date, 250.0);
    txn.actual_amount = Some(250.0);
    txn.actual_date = Some(date);
    TransactionService::add(&mut ledger, txn).unwrap();
    ledger
}

#[test]
fn services_produce_budget_summary() {
    let ledger = prepared_ledger();
    let window = DateWindow::new(
        NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(),
    )
    .expect("window");
    let summary = SummaryService::summarize_window(&ledger, window, BudgetScope::Custom);
    assert_eq!(summary.per_category.len(), 1);
}

#[test]
fn category_crud_roundtrip() {
    let mut ledger = Ledger::new("Categories", BudgetPeriod::monthly());
    let category = Category::new("Subscriptions", CategoryKind::Expense);
    CategoryService::add(&mut ledger, category.clone()).unwrap();

    let mut update = category.clone();
    update.name = "Subscriptions & Media".into();
    CategoryService::edit(&mut ledger, category.id, update).unwrap();

    let fetched = ledger.category(category.id).unwrap();
    assert_eq!(fetched.name, "Subscriptions & Media");

    CategoryService::remove(&mut ledger, category.id).unwrap();
    assert!(ledger.category(category.id).is_none());
}
