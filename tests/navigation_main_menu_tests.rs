mod navigation_support;

use insta::assert_snapshot;
use navigation_support::NavigationTestHarness;
use predicates::prelude::*;

#[test]
fn test_main_menu_initial_layout() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ENTER", "ESC", "END,ENTER"], &[]);
    assert_snapshot!("main_menu_initial_layout", output.stdout);
}

#[test]
fn test_main_menu_has_only_canonical_commands() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ENTER", "ESC", "END,ENTER"], &[]);
    let banned = [
        "new-ledger",
        "save-ledger",
        "load-ledger",
        "backup-ledger",
        "list-backups",
        "restore-ledger",
        "complete",
    ];
    for legacy in banned {
        assert!(
            !output.stdout.contains(legacy),
            "Output should not list legacy command `{legacy}`\n{}",
            output.stdout
        );
    }
}

#[test]
fn test_main_menu_arrow_navigation_cycles_or_bounds() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["UP,ENTER"], &[]);
    assert_snapshot!("main_menu_wraps_on_up", output.stdout);
}

#[test]
fn test_main_menu_esc_no_exit() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ESC", "END,ENTER"], &[]);
    let contains_menu = predicate::str::contains("Main menu");
    assert!(
        output.stdout.matches("Main menu").count() >= 2,
        "Main menu should render twice when ESC pressed\n{}",
        output.stdout
    );
    assert!(contains_menu.eval(&output.stdout));
}

#[test]
fn test_main_menu_banner_displayed() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ENTER", "ESC", "END,ENTER"], &[]);
    assert!(
        output.stdout.contains("no-ledger"),
        "Expected banner to mention no-ledger\n{}",
        output.stdout
    );
}
