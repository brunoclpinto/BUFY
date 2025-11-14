mod navigation_support;

use insta::assert_snapshot;
use navigation_support::NavigationTestHarness;

fn setup_basic_ledger(harness: &NavigationTestHarness, name: &str) {
    let script = format!(
        "ledger new {name} monthly\nledger save-ledger {name}\nexit\n",
        name = name
    );
    harness.run_script(&script);
}

fn setup_transaction_ledger(harness: &NavigationTestHarness, name: &str) {
    let script = format!(
        "ledger new {name} monthly
account add Checking bank
account add Rent expense
transaction add 0 1 2025-01-01 42
ledger save-ledger {name}
exit
",
        name = name
    );
    harness.run_script(&script);
}

#[test]
fn test_account_add_wizard_launches() {
    let harness = NavigationTestHarness::new();
    setup_basic_ledger(&harness, "WizardLedger");
    let output = harness.run_interactive(&["DOWN,ENTER", "ENTER", "ESC"], &["<ESC>"]);
    assert_snapshot!("account_add_wizard_launches", output.stdout);
}

#[test]
fn test_account_add_wizard_cancel_esc() {
    let harness = NavigationTestHarness::new();
    setup_basic_ledger(&harness, "CancelLedger");
    let output = harness.run_interactive(&["DOWN,ENTER", "ENTER", "ESC"], &["<ESC>"]);
    assert!(
        output.stdout.contains("Account creation cancelled."),
        "Expected cancellation notice\n{}",
        output.stdout
    );
}

#[test]
fn test_wizard_does_not_modify_state_on_cancel() {
    let harness = NavigationTestHarness::new();
    setup_basic_ledger(&harness, "StateLedger");
    let _ = harness.run_interactive(&["DOWN,ENTER", "ENTER", "ESC"], &["<ESC>"]);
    let inspection = harness.run_script("account list\nexit\n");
    assert!(
        inspection.stdout.contains("No accounts defined."),
        "Cancelled wizard should not add accounts\n{}",
        inspection.stdout
    );
}

#[test]
fn test_transaction_edit_wizard_launches() {
    let harness = NavigationTestHarness::new();
    setup_transaction_ledger(&harness, "TxnLedger");
    let output = harness.run_interactive_with_env(
        &["DOWN,DOWN,DOWN,ENTER", "DOWN,ENTER", "ESC"],
        &["<ESC>"],
        &[("BUFY_TEST_SELECTIONS", "0")],
    );
    assert_snapshot!("transaction_edit_wizard_launches", output.stdout);
}

#[test]
fn test_wizard_field_prompts_correct_order() {
    let harness = NavigationTestHarness::new();
    setup_basic_ledger(&harness, "OrderLedger");
    let output = harness.run_interactive(&["DOWN,ENTER", "ENTER", "ESC"], &["", "<ESC>"]);
    assert!(
        output.stdout.contains("Step 1 of"),
        "Expected wizard prompt header\n{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("Value cannot be empty"),
        "Expected validation warning for empty input\n{}",
        output.stdout
    );
}
