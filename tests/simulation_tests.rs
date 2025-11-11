use budget_core::{
    core::services::{SummaryService, TransactionService},
    domain::{
        account::{Account, AccountKind},
        ledger::{BudgetScope, DateWindow},
    },
    ledger::{BudgetPeriod, Ledger, Simulation, SimulationChange, SimulationStatus, Transaction},
};
use chrono::NaiveDate;
use uuid::Uuid;

fn ledger_with_simulation() -> Ledger {
    let mut ledger = Ledger::new("Sim", BudgetPeriod::monthly());
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let cash = ledger.add_account(Account::new("Cash", AccountKind::Cash));

    let date = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
    let txn = Transaction::new(checking, cash, None, date, 100.0);
    TransactionService::add(&mut ledger, txn).unwrap();

    let mut simulation = Simulation {
        id: Uuid::new_v4(),
        name: "Raise".into(),
        notes: None,
        status: SimulationStatus::Pending,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        applied_at: None,
        changes: Vec::new(),
    };
    simulation.changes.push(SimulationChange::AddTransaction {
        transaction: Transaction::new(cash, checking, None, date, 25.0),
    });
    ledger.simulations.push(simulation);
    ledger
}

#[test]
fn summarize_simulation_reflects_changes() {
    let ledger = ledger_with_simulation();
    let window = DateWindow::new(
        NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
        NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(),
    )
    .expect("window");
    let impact =
        SummaryService::summarize_simulation(&ledger, "Raise", window, BudgetScope::Custom)
            .expect("summarize simulation");
    assert!(
        impact.delta.budgeted.abs() > f64::EPSILON,
        "expected simulation to affect budget delta"
    );
}
