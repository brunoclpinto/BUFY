mod navigation_support;

use navigation_support::NavigationTestHarness;

#[test]
fn test_legacy_command_new_ledger_not_available() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_script("new-ledger Demo monthly\nexit\n");
    assert!(
        output
            .stdout
            .contains("Unknown command `new-ledger`. Type `help`"),
        "Expected legacy command rejection\n{}",
        output.stdout
    );
}

#[test]
fn test_legacy_list_backups_not_available() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_script("list-backups\nexit\n");
    assert!(
        output
            .stdout
            .contains("Unknown command `list-backups`. Type `help`"),
        "Expected list-backups to be unavailable\n{}",
        output.stdout
    );
}

#[test]
fn test_legacy_complete_not_available() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_script("complete\nexit\n");
    assert!(
        output
            .stdout
            .contains("Unknown command `complete`. Type `help`"),
        "Expected legacy `complete` to be unavailable\n{}",
        output.stdout
    );
}

#[test]
fn test_help_does_not_list_legacy_commands() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_script("help\nexit\n");
    assert!(
        !output.stdout.contains("new-ledger"),
        "Help output should omit legacy commands\n{}",
        output.stdout
    );
    assert!(
        !output.stdout.contains("list-backups"),
        "Help output should omit legacy commands\n{}",
        output.stdout
    );
}
