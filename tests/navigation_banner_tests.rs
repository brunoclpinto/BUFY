mod navigation_support;

use navigation_support::NavigationTestHarness;

#[test]
fn test_banner_no_ledger() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["END,ENTER"], &[]);
    assert!(
        output.stdout.contains("no-ledger"),
        "Expected no-ledger banner\n{}",
        output.stdout
    );
}

#[test]
fn test_banner_with_loaded_ledger() {
    let harness = NavigationTestHarness::new();
    harness.run_script("ledger new Banner monthly\nledger save-ledger Banner\nexit\n");
    let output = harness.run_interactive(&["END,ENTER"], &[]);
    assert!(
        output.stdout.contains("ledger: Banner"),
        "Expected loaded ledger banner\n{}",
        output.stdout
    );
}

#[test]
fn test_banner_with_simulation() {
    let harness = NavigationTestHarness::new();
    harness.run_script(
        "ledger new SimBanner monthly
ledger save-ledger simb
simulation new whatif
ledger save-ledger simb
exit
",
    );
    let output = harness.run_interactive_with_env(
        &["DOWN,DOWN,DOWN,DOWN,ENTER", "DOWN,ENTER", "END,ENTER"],
        &[],
        &[("BUFY_TEST_SELECTIONS", "0")],
    );
    assert!(
        output.stdout.contains("ledger: simb (simulation: whatif)"),
        "Expected simulation banner once simulation entered\n{}",
        output.stdout
    );
}

#[test]
fn test_banner_after_wizard_cancel() {
    let harness = NavigationTestHarness::new();
    harness.run_script("ledger new BannerW monthly\nledger save-ledger BannerW\nexit\n");
    let output = harness.run_interactive(&["DOWN,ENTER", "ENTER", "ESC"], &["<ESC>"]);
    assert!(
        output.stdout.matches("ledger: BannerW").count() >= 2,
        "Expected ledger banner before and after wizard\n{}",
        output.stdout
    );
}

#[test]
fn test_banner_plain_mode() {
    let harness = NavigationTestHarness::new();
    harness.run_script(
        "ledger new Plain monthly
ledger save-ledger Plain
config set theme plain
exit
",
    );
    let output = harness.run_interactive(&["END,ENTER"], &[]);
    assert!(
        output.stdout.contains("ledger: Plain >"),
        "Plain theme should use > arrow\n{}",
        output.stdout
    );
    assert!(
        !output.stdout.contains("⮞"),
        "Plain banner should not contain unicode arrow\n{}",
        output.stdout
    );
}

#[test]
fn test_banner_no_log_noise() {
    let harness = NavigationTestHarness::new();
    let output = harness.run_interactive(&["END,ENTER"], &[]);
    assert!(
        !output.stdout.contains("INFO:"),
        "Banner/menu output should not contain INFO prefix\n{}",
        output.stdout
    );
}
