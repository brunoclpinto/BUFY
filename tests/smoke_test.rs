use budget_core::{
    init,
    ledger::{Account, Ledger, Transaction},
    simulation,
};

#[test]
fn ledger_simulation_smoke() {
    init();

    let mut ledger = Ledger::new();
    let account = Account::new("checking");
    let account_id = account.id;
    ledger.insert_account(account);

    let transaction = Transaction::new(account_id, None, 42_00, "initial deposit");
    ledger.record_transaction(transaction);

    let summary = simulation::summarize(&ledger);
    assert_eq!(summary.transaction_count, 1);
    assert!(ledger.account(account_id).is_ok());
}
