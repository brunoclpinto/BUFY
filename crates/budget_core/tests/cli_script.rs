use assert_cmd::Command;
use budget_core::ledger::{
    transaction::{Recurrence, RecurrenceMode},
    Account, AccountKind, BudgetPeriod, Ledger, TimeInterval, TimeUnit, Transaction,
};
use bufy_storage_json::save_ledger_to_path;
use chrono::NaiveDate;
use predicates::{prelude::PredicateBooleanExt, str::contains};
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[test]
fn script_mode_runs_basic_flow() {
    let home = tempfile::tempdir().unwrap();
    let tmp = NamedTempFile::new().unwrap();
    let input = format!(
        "ledger new Demo every 6 weeks\nledger save {}\nexit\n",
        tmp.path().display()
    );

    let mut cmd = Command::cargo_bin("budget_core_cli").unwrap();
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .env("BUDGET_CORE_HOME", home.path())
        .write_stdin(input)
        .assert()
        .success()
        .stdout(contains("New ledger created"));

    let json = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(json.contains("\"Demo\""));
}

#[test]
fn forecast_command_outputs_projection() {
    let home = tempfile::tempdir().unwrap();
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
    let parent = tmp
        .path()
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&parent).unwrap();
    save_ledger_to_path(&ledger, tmp.path()).unwrap();

    let script = format!(
        "ledger load {}\nforecast 90 days\nexit\n",
        tmp.path().display()
    );

    let mut cmd = Command::cargo_bin("budget_core_cli").unwrap();
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .env("BUDGET_CORE_HOME", home.path())
        .write_stdin(script)
        .assert()
        .success()
        .stdout(contains("Forecast").and(contains("Upcoming projections")));
}

#[test]
fn cli_named_persistence_and_backups() {
    let home = tempfile::tempdir().unwrap();
    let script = "\
ledger new Demo monthly
ledger save-ledger demo
ledger save-ledger demo
ledger list-backups demo
ledger restore 0 demo
exit
";

    let mut cmd = Command::cargo_bin("budget_core_cli").unwrap();
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .env("BUDGET_CORE_HOME", home.path())
        .write_stdin(script)
        .assert()
        .success()
        .stdout(contains("Ledger `demo` saved").and(contains("Ledger `demo` loaded")));
}

#[test]
fn category_budget_cli_flow() {
    let home = tempfile::tempdir().unwrap();
    let script = "\
config set default_budget_period weekly
ledger new Demo monthly
category add Groceries expense
category budget set Groceries 400
category budget show Groceries
category budget set Groceries 600 --period monthly
category budget show Groceries
category budget clear Groceries
category budget show
exit
";

    let mut cmd = Command::cargo_bin("budget_core_cli").unwrap();
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .env("BUDGET_CORE_HOME", home.path())
        .write_stdin(script)
        .assert()
        .success()
        .stdout(
            contains("Budget for `Groceries` set to")
                .and(contains("Weekly"))
                .and(contains("Monthly"))
                .and(contains("Budget cleared for `Groceries`"))
                .and(contains("No category budgets configured")),
        );
}
