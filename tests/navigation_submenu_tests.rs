mod navigation_support;

use insta::assert_snapshot;
use navigation_support::NavigationTestHarness;

#[test]
fn test_ledger_submenu_display() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ENTER", "ESC", "ESC"], &[]);
    assert_snapshot!("ledger_submenu_display", output.stdout);
}

#[test]
fn test_account_submenu_display() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["DOWN,ENTER", "ESC", "ESC"], &[]);
    assert_snapshot!("account_submenu_display", output.stdout);
}

#[test]
fn test_simulation_submenu_display() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["DOWN,DOWN,DOWN,DOWN,ENTER", "ESC", "ESC"], &[]);
    assert_snapshot!("simulation_submenu_display", output.stdout);
}

#[test]
fn test_submenu_esc_returns_to_main_menu() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["DOWN,ENTER", "ESC", "ESC"], &[]);
    let occurrences = output.stdout.matches("Version").count();
    assert!(
        occurrences >= 2,
        "Main menu should render again after ESC:\n{}",
        output.stdout
    );
}

#[test]
fn test_submenu_selection_triggers_handler() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ENTER", "DOWN,DOWN,DOWN,DOWN,DOWN,ENTER", "ESC"], &[]);
    assert!(
        output
            .stdout
            .contains("No ledger currently loaded. Load or create a ledger to view backups."),
        "Expected ledger overview handler output:\n{}",
        output.stdout
    );
}

#[test]
fn test_submenu_invalid_input_handling() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(
        &["ENTER", "DOWN,DOWN,DOWN,DOWN,DOWN,DOWN,ENTER", "ESC"],
        &[],
    );
    assert!(
        output
            .stdout
            .contains("Ledger deletion workflow is not available yet."),
        "Expected guard message for unavailable action:\n{}",
        output.stdout
    );
}
