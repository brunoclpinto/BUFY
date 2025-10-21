use budget_core::{
    init,
    ledger::{Account, AccountKind, BudgetPeriod, Ledger, TimeInterval, TimeUnit, Transaction},
    simulation,
};
use chrono::NaiveDate;

#[test]
fn ledger_simulation_smoke() {
    init();

    let mut ledger = Ledger::new("SmokeTest", BudgetPeriod::default());
    let from_account = ledger.add_account(Account::new("checking", AccountKind::Bank));
    let to_account = ledger.add_account(Account::new("savings", AccountKind::Savings));

    let mut transaction = Transaction::new(
        from_account,
        to_account,
        None,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        42.0,
    );
    transaction.recurrence = Some(budget_core::ledger::Recurrence {
        interval: TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        mode: budget_core::ledger::RecurrenceMode::FixedSchedule,
    });
    ledger.add_transaction(transaction);

    let summary = simulation::summarize(&ledger);
    assert_eq!(summary.transaction_count, 1);
    assert!(ledger.account(from_account).is_some());
}
