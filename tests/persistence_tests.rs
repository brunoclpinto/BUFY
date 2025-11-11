mod common;

use budget_core::ledger::{BudgetPeriod, Ledger};

#[test]
fn ledger_roundtrip_preserves_name() {
    let (mut manager, _) = common::setup_test_env();
    let ledger = Ledger::new("Household", BudgetPeriod::monthly());
    manager.set_current(ledger, None, Some("household".into()));
    manager.save_as("household").expect("save ledger");

    manager.clear();
    let report = manager.load("household").expect("reload ledger");
    assert_eq!(report.name.as_deref(), Some("household"));
    manager
        .with_current(|ledger| {
            assert_eq!(ledger.name, "Household");
        })
        .expect("ledger present");
}
