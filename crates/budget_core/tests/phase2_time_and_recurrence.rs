use budget_core::ledger::{
    transaction::{Recurrence, RecurrenceMode},
    Account, AccountKind, BudgetPeriod, DateWindow, Ledger, TimeInterval, TimeUnit, Transaction,
};
use budget_core::storage::json_backend::JsonStorage;
use chrono::{NaiveDate, TimeZone, Utc};
use serde_json::Value;
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[test]
fn test_timeinterval_next_date() {
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

    let day = TimeInterval {
        every: 3,
        unit: TimeUnit::Day,
    };
    assert_eq!(
        day.next_date(start),
        NaiveDate::from_ymd_opt(2025, 1, 4).unwrap()
    );

    let week = TimeInterval {
        every: 2,
        unit: TimeUnit::Week,
    };
    assert_eq!(
        week.next_date(start),
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()
    );

    let month = TimeInterval {
        every: 1,
        unit: TimeUnit::Month,
    };
    assert_eq!(
        month.next_date(start),
        NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()
    );

    let year = TimeInterval {
        every: 1,
        unit: TimeUnit::Year,
    };
    assert_eq!(
        year.next_date(start),
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
    );
}

#[test]
fn test_recurrence_fixed_vs_afterlast() {
    let interval = TimeInterval {
        every: 1,
        unit: TimeUnit::Month,
    };
    let last_scheduled = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let last_performed = Some(NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());

    let fixed = Recurrence::new(
        last_scheduled,
        interval.clone(),
        RecurrenceMode::FixedSchedule,
    );
    let after = Recurrence::new(last_scheduled, interval, RecurrenceMode::AfterLastPerformed);

    assert_eq!(
        fixed.next_occurrence(last_scheduled, last_performed),
        NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()
    );
    assert_eq!(
        after.next_occurrence(last_scheduled, last_performed),
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap()
    );
}

#[test]
fn test_recurrence_reset_behavior() {
    let recurrence = Recurrence::new(
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        TimeInterval {
            every: 2,
            unit: TimeUnit::Month,
        },
        RecurrenceMode::AfterLastPerformed,
    );

    let next = recurrence.next_occurrence(
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap()),
    );

    assert_eq!(next, NaiveDate::from_ymd_opt(2025, 6, 1).unwrap());
}

#[test]
fn test_serialization_roundtrip() {
    let mut ledger = Ledger::new("Phase2", BudgetPeriod::default());
    let from = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Savings", AccountKind::Savings));

    let mut transaction = Transaction::new(
        from,
        to,
        None,
        NaiveDate::from_ymd_opt(2025, 5, 5).unwrap(),
        1200.55,
    );
    transaction.set_recurrence(Some(Recurrence::new(
        NaiveDate::from_ymd_opt(2025, 5, 5).unwrap(),
        TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(transaction);

    // Adjust timestamps to deterministic values for comparison.
    ledger.created_at = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    ledger.updated_at = ledger.created_at;

    let tmp = NamedTempFile::new().unwrap();
    let parent = tmp
        .path()
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let store = JsonStorage::new(Some(parent), Some(2)).unwrap();
    let snapshot = ledger.clone();
    store.save_to_path(&snapshot, tmp.path()).unwrap();
    let loaded = store.load_from_path(tmp.path()).unwrap();

    let original_json: Value = serde_json::to_value(&ledger).unwrap();
    let loaded_json: Value = serde_json::to_value(&loaded).unwrap();
    assert_eq!(original_json, loaded_json);
}

#[test]
fn test_label_generation() {
    let monthly = TimeInterval {
        every: 1,
        unit: TimeUnit::Month,
    };
    assert_eq!(monthly.label(), "Monthly");

    let biweekly = TimeInterval {
        every: 2,
        unit: TimeUnit::Week,
    };
    assert_eq!(biweekly.label(), "Every 2 Weeks");
}

#[test]
fn test_materialize_and_forecast_flow() {
    let mut ledger = Ledger::new("Forecast", BudgetPeriod::default());
    let from = ledger.add_account(Account::new("Operating", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Rent", AccountKind::ExpenseDestination));

    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let mut template = Transaction::new(from, to, None, start, 1500.0);
    template.set_recurrence(Some(Recurrence::new(
        start,
        TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(template);

    let reference = NaiveDate::from_ymd_opt(2025, 3, 5).unwrap();
    let created = ledger.materialize_due_recurrences(reference);
    assert_eq!(created, 2, "Expected Feb and Mar instances to materialize");
    assert_eq!(ledger.transactions.len(), 3);
    assert!(ledger
        .transactions
        .iter()
        .any(|txn| txn.recurrence.is_none() && txn.recurrence_series().is_some()));

    let window = DateWindow::new(
        NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
    )
    .unwrap();
    let report = ledger
        .forecast_window_report(window, reference, None)
        .expect("forecast");
    assert!(
        report.forecast.transactions.len() >= 2,
        "should generate at least two future projections"
    );
}

#[test]
fn materialize_handles_backlog_across_multiple_periods() {
    let mut ledger = Ledger::new("Backlog", BudgetPeriod::default());
    let from = ledger.add_account(Account::new("Operating", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Rent", AccountKind::ExpenseDestination));

    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let mut template = Transaction::new(from, to, None, start, 1500.0);
    template.set_recurrence(Some(Recurrence::new(
        start,
        TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(template);

    let reference = NaiveDate::from_ymd_opt(2025, 5, 1).unwrap();
    let created = ledger.materialize_due_recurrences(reference);
    assert_eq!(created, 4, "expected Feb-May materializations");

    let generated: Vec<_> = ledger
        .transactions
        .iter()
        .filter(|txn| txn.recurrence.is_none() && txn.recurrence_series_id.is_some())
        .collect();
    assert_eq!(generated.len(), 4);

    let expected_dates: std::collections::BTreeSet<_> = [
        NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
    ]
    .into_iter()
    .collect();
    let actual_dates: std::collections::BTreeSet<_> =
        generated.iter().map(|txn| txn.scheduled_date).collect();
    assert_eq!(actual_dates, expected_dates);

    let template = ledger
        .transactions
        .iter()
        .find(|txn| txn.recurrence.is_some())
        .expect("template");
    let next_due = template
        .recurrence
        .as_ref()
        .and_then(|rule| rule.next_scheduled);
    assert_eq!(
        next_due,
        Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
        "metadata should advance to the next future period"
    );
}
