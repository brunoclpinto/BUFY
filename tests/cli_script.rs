use assert_cmd::Command;
use predicates::str::contains;
use tempfile::NamedTempFile;

#[test]
fn script_mode_runs_basic_flow() {
    let tmp = NamedTempFile::new().unwrap();
    let input = format!(
        "new-ledger Demo monthly\nsave {}\nexit\n",
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
