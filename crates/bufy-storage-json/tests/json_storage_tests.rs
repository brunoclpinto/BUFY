use bufy_core::storage::LedgerStorage;
use bufy_domain::{Ledger, LedgerBudgetPeriod};
use bufy_storage_json::JsonLedgerStorage;
use tempfile::tempdir;

#[test]
fn json_storage_can_save_and_load_ledger() {
    let dir = tempdir().expect("tempdir");
    let storage = JsonLedgerStorage::new(dir.path().join("ledgers"), dir.path().join("backups"))
        .expect("create storage");

    let ledger = Ledger::new("StorageTest", LedgerBudgetPeriod::monthly());

    storage
        .save_ledger("test-ledger", &ledger)
        .expect("save ledger");
    let loaded = storage.load_ledger("test-ledger").expect("load ledger");

    assert_eq!(loaded.name, "StorageTest");
    assert_eq!(loaded.budget_period, ledger.budget_period);
}

#[test]
fn json_storage_creates_and_restores_backups() {
    let dir = tempdir().expect("tempdir");
    let storage = JsonLedgerStorage::new(dir.path().join("ledgers"), dir.path().join("backups"))
        .expect("create storage");

    let ledger = Ledger::new("BackupTest", LedgerBudgetPeriod::monthly());
    storage
        .save_ledger("backup-ledger", &ledger)
        .expect("save ledger");

    let info = storage
        .backup_ledger("backup-ledger", &ledger, None)
        .expect("create backup");

    let backups = storage.list_backups("backup-ledger").expect("list backups");
    assert!(
        backups.iter().any(|entry| entry.id == info.id),
        "backup list should include created backup"
    );

    let restored = storage.restore_backup(&info).expect("restore backup");
    assert_eq!(restored.name, ledger.name);
}
