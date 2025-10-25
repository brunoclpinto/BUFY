use assert_cmd::Command;
use budget_core::{
    ledger::{
        transaction::{Recurrence, RecurrenceMode},
        Account, AccountKind, BudgetPeriod, Ledger, TimeInterval, TimeUnit, Transaction,
    },
    utils::persistence,
};
use chrono::NaiveDate;
use predicates::{prelude::PredicateBooleanExt, str::contains};
use tempfile::NamedTempFile;

#[test]
fn script_mode_runs_basic_flow() {
    let tmp = NamedTempFile::new().unwrap();
    let input = format!(
        "new-ledger Demo every 6 weeks\nsave {}\nexit\n",
        tmp.path().display()
    );

    let mut cmd = Command::cargo_bin("budget_core_cli").unwrap();
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(contains("New ledger created"));

    let json = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(json.contains("\"Demo\""));
}

#[test]
fn forecast_command_outputs_projection() {
    let mut ledger = Ledger::new("CLI Forecast", BudgetPeriod::default());
    let from = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let to = ledger.add_account(Account::new("Landlord", AccountKind::ExpenseDestination));
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let mut txn = Transaction::new(from, to, None, start, 900.0);
    txn.set_recurrence(Some(Recurrence::new(
        start,
        TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        },
        RecurrenceMode::FixedSchedule,
    )));
    ledger.add_transaction(txn);

    let tmp = NamedTempFile::new().unwrap();
    persistence::save_ledger_to_file(&ledger, tmp.path()).unwrap();

    let script = format!("load {}\nforecast 90 days\nexit\n", tmp.path().display());

    let mut cmd = Command::cargo_bin("budget_core_cli").unwrap();
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .write_stdin(script)
        .assert()
        .success()
        .stdout(contains("Forecast").and(contains("Upcoming projections")));
}
