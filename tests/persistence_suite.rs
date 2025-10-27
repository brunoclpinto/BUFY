use budget_core::{
    ledger::{Account, AccountKind, BudgetPeriod, Ledger, Transaction},
    utils::persistence::LedgerStore,
};
use chrono::NaiveDate;
use serde_json;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn sample_transaction(ledger: &mut Ledger, amount: f64) {
    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let txn = Transaction::new(
        checking,
        savings,
        None,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        amount,
    );
    ledger.add_transaction(txn);
}

fn tmp_path_for(path: &Path) -> std::path::PathBuf {
    let mut tmp = path.to_path_buf();
    let ext = match path.extension().and_then(|ext| ext.to_str()) {
        Some(existing) => format!("{}.tmp", existing),
        None => String::from("tmp"),
    };
    tmp.set_extension(ext);
    tmp
}

#[test]
fn atomic_save_failure_preserves_original_file() {
    let temp = tempdir().unwrap();
    let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(2)).unwrap();

    let mut ledger = Ledger::new("Reliable", BudgetPeriod::default());
    sample_transaction(&mut ledger, 42.0);

    let path = store
        .save_named(&mut ledger, "reliable-ledger")
        .expect("initial save");
    let original = fs::read_to_string(&path).expect("read original file");

    // Create directory that collides with the temp file name to force File::create to fail.
    let tmp_path = tmp_path_for(&path);
    fs::create_dir_all(&tmp_path).unwrap();

    // Mutate ledger to ensure new JSON would differ if the save succeeded.
    sample_transaction(&mut ledger, 99.0);
    let result = store.save_to_path(&mut ledger, &path);
    assert!(
        result.is_err(),
        "expected save_to_path to fail when temp path is a directory"
    );

    let current = fs::read_to_string(&path).expect("read after failure");
    assert_eq!(
        current, original,
        "atomic save failure must not corrupt the original file"
    );

    let backups = store.list_backups("reliable-ledger").unwrap();
    assert!(
        !backups.is_empty(),
        "backup should be created before attempting the write"
    );
    assert!(
        backups.iter().any(|info| {
            info.path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with(".json.bak"))
                .unwrap_or(false)
        }),
        "backup filename should retain .json.bak suffix"
    );

    let _ = fs::remove_dir_all(&tmp_path);
}

#[test]
fn store_creates_and_restores_backups() {
    let temp = tempdir().unwrap();
    let mut ledger = Ledger::new("Household", BudgetPeriod::default());
    sample_transaction(&mut ledger, 50.0);

    let store = LedgerStore::new(Some(temp.path().to_path_buf()), Some(5)).unwrap();
    store
        .save_named(&mut ledger, "family-budget")
        .expect("initial save");

    // Modify ledger and save again to trigger a backup.
    sample_transaction(&mut ledger, 75.0);
    store
        .save_named(&mut ledger, "family-budget")
        .expect("second save");

    let backups = store.list_backups("family-budget").unwrap();
    assert!(
        !backups.is_empty(),
        "expected at least one backup after second save"
    );

    // Restore the oldest backup (should represent the first save).
    let oldest = backups.last().unwrap().path.clone();
    let snapshot = std::fs::read_to_string(&oldest).unwrap();
    let ledger_snapshot: Ledger = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(ledger_snapshot.transactions.len(), 1);
    store
        .restore_backup("family-budget", &oldest)
        .expect("restore");
    let restored_raw = std::fs::read_to_string(store.ledger_path("family-budget")).unwrap();
    let restored_disk: Ledger = serde_json::from_str(&restored_raw).unwrap();
    assert_eq!(restored_disk.transactions.len(), 1);
    let restored = store
        .load_named("family-budget")
        .expect("load restored ledger")
        .ledger;
    assert_eq!(
        restored.transactions.len(),
        1,
        "restored ledger should match the first snapshot"
    );
}
