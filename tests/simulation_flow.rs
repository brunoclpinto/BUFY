use budget_core::ledger::{
    account::AccountKind, category::CategoryKind, Account, BudgetPeriod, Ledger, SimulationStatus,
    TimeInterval, TimeUnit, Transaction,
};
use chrono::NaiveDate;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn simulation_round_trip_and_apply() {
    let mut ledger = Ledger::new(
        "Demo",
        BudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        }),
    );
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    ledger.add_category(budget_core::ledger::Category::new(
        "Utilities",
        CategoryKind::Expense,
    ));

    let txn = Transaction::new(checking, savings, None, date(2025, 1, 5), 100.0);
    ledger.add_transaction(txn);

    ledger
        .create_simulation("WhatIf", Some("Test".into()))
        .unwrap();
    let simulated = Transaction::new(checking, savings, None, date(2025, 1, 10), 250.0);
    ledger
        .add_simulation_transaction("WhatIf", simulated)
        .unwrap();

    let reference = date(2025, 1, 15);
    let window = ledger.budget_window_for(reference);
    let scope = window.scope(reference);
    let impact = ledger
        .summarize_simulation_in_window("WhatIf", window, scope)
        .expect("impact");
    assert!(impact.simulated.totals.budgeted > impact.base.totals.budgeted);

    ledger.apply_simulation("WhatIf").unwrap();
    let sim = ledger.simulation("WhatIf").unwrap();
    assert_eq!(sim.status, SimulationStatus::Applied);
    assert!(
        ledger
            .summarize_period_containing(date(2025, 1, 20))
            .totals
            .budgeted
            >= 350.0
    );
}

#[test]
fn simulations_survive_serialization() {
    let mut ledger = Ledger::new(
        "Persisted",
        BudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        }),
    );
    ledger.create_simulation("PlanA", None).unwrap();
    let json = serde_json::to_string(&ledger).unwrap();
    let roundtrip: Ledger = serde_json::from_str(&json).unwrap();
    assert!(roundtrip
        .simulations()
        .iter()
        .any(|sim| sim.name == "PlanA" && sim.status == SimulationStatus::Pending));
}
