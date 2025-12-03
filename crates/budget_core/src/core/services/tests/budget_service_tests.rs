use chrono::NaiveDate;

use crate::core::services::BudgetService;
use bufy_domain::{
    account::{Account, AccountKind},
    category::{Category, CategoryKind},
    BudgetPeriod as CategoryBudgetPeriod,
};
use crate::ledger::{BudgetPeriod as LedgerBudgetPeriod, Ledger, Transaction};

#[test]
fn summarize_window_matches_ledger_internal() {
    let ledger = sample_ledger();
    let reference = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let window = ledger.budget_window_for(reference);
    let scope = window.scope(reference);
    let via_service = BudgetService::summarize_window_scope(&ledger, window, scope);
    let via_ledger = ledger.summarize_window_scope(window, scope);
    assert_eq!(via_service.totals.budgeted, via_ledger.totals.budgeted);
}

#[test]
fn category_budget_status_reflects_assignment() {
    let mut ledger = sample_ledger();
    let mut entertainment = Category::new("Entertainment", CategoryKind::Expense);
    entertainment.set_budget(200.0, CategoryBudgetPeriod::Monthly, None);
    let entertainment_id = entertainment.id;
    ledger.add_category(entertainment);

    let txn_date = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
    let checking = ledger.accounts[0].id;
    let savings = ledger.accounts[1].id;
    let txn = Transaction::new(checking, savings, Some(entertainment_id), txn_date, 50.0);
    ledger.add_transaction(txn);

    let window = ledger.budget_window_for(txn_date);
    let scope = window.scope(txn_date);
    let status =
        BudgetService::category_budget_status(&ledger, entertainment_id, window, scope).unwrap();
    assert!(status.budget.is_some());
    assert_eq!(status.totals.budgeted, 50.0);
}

fn sample_ledger() -> Ledger {
    let mut ledger = Ledger::new("ServiceBudget", LedgerBudgetPeriod::monthly());
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let txn = Transaction::new(
        checking,
        savings,
        None,
        NaiveDate::from_ymd_opt(2024, 1, 5).unwrap(),
        125.0,
    );
    ledger.add_transaction(txn);
    ledger
}
