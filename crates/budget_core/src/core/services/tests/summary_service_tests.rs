use chrono::NaiveDate;

use crate::core::services::SummaryService;
use bufy_domain::{
    account::{Account, AccountKind},
    transaction::Transaction,
};
use crate::ledger::{BudgetPeriod, Ledger};

#[test]
fn current_totals_matches_ledger() {
    let ledger = sample_ledger();
    let summary = SummaryService::current_totals(&ledger);
    let direct = ledger.summarize_current_period();
    assert_eq!(summary.totals.budgeted, direct.totals.budgeted);
}

#[test]
fn summarize_window_returns_budget_summary() {
    let ledger = sample_ledger();
    let window = ledger.budget_window_for(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let scope = window.scope(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let summary = SummaryService::summarize_window(&ledger, window, scope);
    assert!(summary.totals.budgeted > 0.0);
}

#[test]
fn forecast_window_matches_ledger() {
    let ledger = sample_ledger();
    let today = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let window = ledger.budget_window_for(today);
    let report_a = SummaryService::forecast_window(&ledger, window, today, None).unwrap();
    let report_b = ledger.forecast_window_report(window, today, None).unwrap();
    assert_eq!(report_a.scope, report_b.scope);
}

fn sample_ledger() -> Ledger {
    let mut ledger = Ledger::new("Demo", BudgetPeriod::monthly());
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let txn = Transaction::new(
        checking,
        savings,
        None,
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        100.0,
    );
    ledger.add_transaction(txn);
    ledger
}
