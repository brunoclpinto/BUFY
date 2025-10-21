use budget_core::ledger::{
    transaction::{Recurrence, RecurrenceMode},
    Account, AccountKind, BudgetPeriod, Ledger, TimeInterval, TimeUnit, Transaction,
};
use budget_core::utils::persistence;
use chrono::{NaiveDate, TimeZone, Utc};
use serde_json::Value;
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

    let fixed = Recurrence {
        interval: interval.clone(),
        mode: RecurrenceMode::FixedSchedule,
    };
    let after = Recurrence {
        interval,
        mode: RecurrenceMode::AfterLastPerformed,
    };

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
    let recurrence = Recurrence {
        interval: TimeInterval {
            every: 2,
            unit: TimeUnit::Month,
        },
        mode: RecurrenceMode::AfterLastPerformed,
    };

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
    transaction.recurrence = Some(Recurrence {
        interval: TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        mode: RecurrenceMode::FixedSchedule,
    });
    ledger.add_transaction(transaction);

    // Adjust timestamps to deterministic values for comparison.
    ledger.created_at = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    ledger.updated_at = ledger.created_at;

    let tmp = NamedTempFile::new().unwrap();
    persistence::save_ledger_to_file(&ledger, tmp.path()).unwrap();
    let loaded = persistence::load_ledger_from_file(tmp.path()).unwrap();

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
