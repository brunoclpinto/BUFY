use assert_cmd::Command;
use predicates::prelude::*;
use std::error::Error;

#[test]
fn script_mode_creates_and_lists_accounts() -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::cargo_bin("budget_core_cli")?;
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1")
        .write_stdin("ledger new Demo monthly\nlist accounts\nexit\n")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("New ledger created.")
                .and(predicate::str::contains("No accounts in this ledger.")),
        );
    Ok(())
}
