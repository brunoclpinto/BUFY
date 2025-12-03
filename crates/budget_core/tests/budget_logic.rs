use budget_core::ledger::{
    account::AccountKind, category::CategoryKind, Account, BudgetPeriod, BudgetScope, BudgetStatus,
    Category, DateWindow, Ledger, TimeInterval, TimeUnit, Transaction,
};
use bufy_core::BudgetService;
use chrono::NaiveDate;

fn sample_date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn summarizes_budgeted_vs_real_by_period() {
    let mut ledger = Ledger::new(
        "Household",
        BudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        }),
    );

    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let groceries = ledger.add_category(Category::new("Groceries", CategoryKind::Expense));

    let mut txn1 = Transaction::new(
        checking,
        savings,
        Some(groceries),
        sample_date(2025, 1, 5),
        200.0,
    );
    txn1.actual_date = Some(sample_date(2025, 1, 6));
    txn1.actual_amount = Some(180.0);
    ledger.add_transaction(txn1);

    let txn2 = Transaction::new(
        checking,
        savings,
        Some(groceries),
        sample_date(2025, 1, 20),
        150.0,
    );
    // pending actuals
    ledger.add_transaction(txn2);

    let mut txn3 = Transaction::new(
        checking,
        savings,
        Some(groceries),
        sample_date(2025, 2, 2),
        120.0,
    );
    txn3.actual_date = Some(sample_date(2025, 2, 3));
    txn3.actual_amount = Some(140.0);
    ledger.add_transaction(txn3);

    let january = BudgetService::summarize_period_containing(&ledger, sample_date(2025, 1, 15));
    assert_eq!(january.totals.budgeted, 350.0);
    assert_eq!(january.totals.real, 180.0);
    assert!(january.totals.incomplete);
    assert_eq!(january.incomplete_transactions, 1);
    assert_eq!(january.per_category.len(), 1);
    assert_eq!(january.per_category[0].totals.budgeted, 350.0);

    let february = BudgetService::summarize_period_containing(&ledger, sample_date(2025, 2, 10));
    assert_eq!(february.totals.budgeted, 120.0);
    assert_eq!(february.totals.real, 140.0);
    assert_eq!(february.totals.status, BudgetStatus::OverBudget);
}

#[test]
fn summarizes_custom_range() {
    let mut ledger = Ledger::new(
        "Custom",
        BudgetPeriod(TimeInterval {
            every: 2,
            unit: TimeUnit::Week,
        }),
    );
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));

    let mut txn = Transaction::new(checking, checking, None, sample_date(2025, 3, 1), 50.0);
    txn.actual_date = Some(sample_date(2025, 3, 1));
    txn.actual_amount = Some(55.0);
    ledger.add_transaction(txn);

    let window =
        DateWindow::new(sample_date(2025, 3, 1), sample_date(2025, 3, 31)).expect("valid window");
    let summary = BudgetService::summarize_window_scope(&ledger, window, BudgetScope::Custom);
    assert_eq!(summary.totals.budgeted, 50.0);
    assert_eq!(summary.totals.real, 55.0);
    assert_eq!(summary.scope, BudgetScope::Custom);
}
