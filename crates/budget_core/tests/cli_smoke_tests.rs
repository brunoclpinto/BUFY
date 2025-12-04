use assert_cmd::Command;
use predicates::{prelude::PredicateBooleanExt, str::contains};

const BIN_NAME: &str = "budget_core_cli";

fn script_command() -> Command {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary exists");
    cmd.env("BUDGET_CORE_CLI_SCRIPT", "1");
    cmd
}

#[test]
fn cli_help_command_prints_overview() {
    script_command()
        .write_stdin("help\nexit\n")
        .assert()
        .success()
        .stdout(contains("help").or(contains("Available commands")));
}

#[test]
fn cli_version_command_prints_version_info() {
    script_command()
        .write_stdin("version\nexit\n")
        .assert()
        .success()
        .stdout(contains("version").or(contains("Budget Core")));
}
