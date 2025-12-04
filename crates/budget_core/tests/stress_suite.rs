use budget_core::cli::system_clock::SystemClock;
use budget_core::ledger::{
    account::AccountKind, category::CategoryKind, Account, BudgetPeriod, DateWindow, Ledger,
    LedgerExt, TimeInterval, TimeUnit, Transaction,
};
use bufy_core::storage::LedgerStorage;
use bufy_storage_json::JsonLedgerStorage as JsonStorage;
use chrono::{Duration, NaiveDate};
use tempfile::tempdir;

fn seed_ledger() -> Ledger {
    let mut ledger = Ledger::new(
        "Stress Harness",
        BudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        }),
    );
    let employer = ledger.add_account(Account::new("Employer", AccountKind::IncomeSource));
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let landlord = ledger.add_account(Account::new("Landlord", AccountKind::ExpenseDestination));
    let grocer = ledger.add_account(Account::new("Grocer", AccountKind::ExpenseDestination));

    let housing = ledger.add_category(budget_core::ledger::Category::new(
        "Housing",
        CategoryKind::Expense,
    ));
    let groceries = ledger.add_category(budget_core::ledger::Category::new(
        "Groceries",
        CategoryKind::Expense,
    ));
    let income = ledger.add_category(budget_core::ledger::Category::new(
        "Income",
        CategoryKind::Income,
    ));

    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

    // Recurring rent template.
    let mut rent = Transaction::new(checking, landlord, Some(housing), start, 1500.0);
    rent.set_recurrence(Some(budget_core::ledger::transaction::Recurrence::new(
        start,
        TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        budget_core::ledger::transaction::RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(rent);

    // Weekly groceries recurrence.
    let mut groceries_txn = Transaction::new(checking, grocer, Some(groceries), start, 200.0);
    groceries_txn.set_recurrence(Some(budget_core::ledger::transaction::Recurrence::new(
        start,
        TimeInterval {
            every: 1,
            unit: TimeUnit::Week,
        },
        budget_core::ledger::transaction::RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(groceries_txn);

    // Salary income transactions for three months.
    for month in 0..3 {
        let payday = TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        }
        .add_to(start, month);
        let mut paycheck = Transaction::new(employer, checking, Some(income), payday, -4000.0);
        paycheck.actual_amount = Some(-4000.0);
        paycheck.actual_date = Some(payday);
        ledger.add_transaction(paycheck);
    }

    ledger
}

#[test]
fn stress_repeated_save_load_and_forecast_cycles() {
    let tmp = tempdir().unwrap();
    let store =
        JsonStorage::with_retention(tmp.path().join("ledgers"), tmp.path().join("backups"), 3)
            .unwrap();
    let mut ledger = seed_ledger();

    // Simulation with an additional expense to exercise overlay calculations.
    let clock = SystemClock;
    ledger
        .create_simulation("Scenario", Some("Stress overlay".into()), &clock)
        .unwrap();
    let scenario_txn = Transaction::new(
        ledger.accounts[1].id, // checking
        ledger.accounts[2].id, // landlord
        ledger
            .categories
            .iter()
            .find(|c| c.name == "Housing")
            .map(|c| c.id),
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
        100.0,
    );
    ledger
        .add_simulation_transaction("Scenario", scenario_txn)
        .unwrap();

    store
        .save_ledger("stress-ledger", &ledger)
        .expect("initial save");

    let mut reference = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    for step in 0..24 {
        reference += Duration::days(15);
        let created = ledger.materialize_due_recurrences(reference);
        if created > 0 {
            for txn in ledger.transactions.iter_mut() {
                if txn.actual_date.is_none() && txn.scheduled_date <= reference {
                    txn.actual_date = Some(txn.scheduled_date);
                    txn.actual_amount = Some(txn.budgeted_amount);
                }
            }
        }

        let summary = ledger.summarize_period_containing(reference);
        assert!(
            summary.totals.budgeted.is_finite() && summary.totals.real.is_finite(),
            "summary totals should remain finite"
        );

        let window = DateWindow::new(reference, reference + Duration::days(60)).unwrap();
        let forecast = ledger
            .forecast_window_report(window, reference, None)
            .expect("forecast report");
        assert!(
            forecast.forecast.totals.generated <= 1024,
            "forecast should respect max occurrence bound"
        );

        if let Ok(impact) = ledger.summarize_simulation_current("Scenario", &clock) {
            assert_eq!(impact.simulation_name, "Scenario");
            assert!(
                impact.simulated.totals.budgeted >= impact.base.totals.budgeted - 200.0,
                "simulation budget should not drift unexpectedly"
            );
        }

        // Periodically mutate the simulation change set to exercise overlays.
        if step % 6 == 5 {
            let extra = Transaction::new(
                ledger.accounts[1].id,
                ledger.accounts[3].id,
                ledger
                    .categories
                    .iter()
                    .find(|c| c.name == "Groceries")
                    .map(|c| c.id),
                reference,
                45.0,
            );
            ledger
                .add_simulation_transaction("Scenario", extra)
                .unwrap();
        }

        store
            .save_ledger("stress-ledger", &ledger)
            .expect("save iteration");
        let reloaded = store.load_ledger("stress-ledger").expect("reload ledger");
        assert_eq!(
            reloaded.transactions.len(),
            ledger.transactions.len(),
            "transaction count should persist across reload"
        );
        ledger = reloaded;
    }
}
