mod navigation_support;

use insta::assert_snapshot;
use navigation_support::NavigationTestHarness;

#[test]
fn test_main_menu_initial_layout() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ESC"], &[]);
    assert_snapshot!("main_menu_initial_layout", output.stdout);
}

#[test]
fn test_main_menu_has_only_canonical_commands() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ESC"], &[]);
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
    let output = harness.run_interactive(&["UP,ESC"], &[]);
    assert_snapshot!("main_menu_wraps_on_up", output.stdout);
}

#[test]
fn test_main_menu_esc_exits() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ESC"], &[]);
    assert!(
        output.stdout.contains("Exiting shell."),
        "ESC should exit shell immediately\n{}",
        output.stdout
    );
}

#[test]
fn test_main_menu_banner_displayed() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ESC"], &[]);
    assert!(
        output.stdout.contains("no-ledger"),
        "Expected banner to mention no-ledger\n{}",
        output.stdout
    );
}

#[test]
fn test_main_menu_alignment_rules() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["ESC"], &[]);
    let menu_lines: Vec<&str> = output
        .stdout
        .lines()
        .filter(|line| line.starts_with('>') || line.starts_with('-'))
        .collect();
    assert!(
        !menu_lines.is_empty(),
        "Menu lines missing:\n{}",
        output.stdout
    );

    for line in &menu_lines {
        let first = line.chars().next().unwrap();
        assert!(
            !first.is_whitespace(),
            "Menu line must not start with whitespace: `{}`",
            line
        );
    }

    let expected_desc_col = menu_lines[0]
        .rfind("  ")
        .expect("menu line should contain separator");
    for line in &menu_lines {
        let idx = line
            .rfind("  ")
            .expect("menu line should contain separator");
        assert_eq!(
            idx, expected_desc_col,
            "Descriptions should align consistently. Expected column {expected_desc_col}, got {idx} on `{}`",
            line
        );
    }
}
