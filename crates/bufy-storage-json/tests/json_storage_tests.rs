use bufy_core::storage::LedgerStorage;
use bufy_domain::{Ledger, LedgerBudgetPeriod};
use bufy_storage_json::{JsonLedgerStorage, StoragePaths};
use serde_json::to_string;
use std::fs;
use tempfile::tempdir;

#[test]
fn json_storage_can_save_and_load_ledger() {
    let dir = tempdir().expect("tempdir");
    let paths = StoragePaths {
        ledger_root: dir.path().join("ledgers"),
        backup_root: dir.path().join("backups"),
    };
    let storage = JsonLedgerStorage::new(paths).expect("create storage");

    let ledger = Ledger::new("StorageTest", LedgerBudgetPeriod::monthly());

    storage
        .save_ledger("test-ledger", &ledger)
        .expect("save ledger");
    let loaded = storage.load_ledger("test-ledger").expect("load ledger");

    assert_eq!(loaded.name, "StorageTest");
    assert_eq!(loaded.budget_period, ledger.budget_period);
    let path = storage.ledger_path("test-ledger");
    assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("bfy"));
    assert!(path.exists());
}

#[test]
fn json_storage_creates_and_restores_backups() {
    let dir = tempdir().expect("tempdir");
    let paths = StoragePaths {
        ledger_root: dir.path().join("ledgers"),
        backup_root: dir.path().join("backups"),
    };
    let storage = JsonLedgerStorage::new(paths.clone()).expect("create storage");

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
    assert_eq!(
        info.path.extension().and_then(|ext| ext.to_str()),
        Some("bbfy")
    );
    let backup_path = storage.ledger_path("backup-ledger");
    let backup_slug = backup_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("slug");
    assert_eq!(
        info.path
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_default(),
        paths.backup_root.join(format!("{backup_slug}-backups"))
    );
}

#[test]
fn json_storage_loads_legacy_json_ledgers() {
    let dir = tempdir().expect("tempdir");
    let paths = StoragePaths {
        ledger_root: dir.path().join("ledgers"),
        backup_root: dir.path().join("backups"),
    };
    let storage = JsonLedgerStorage::new(paths.clone()).expect("create storage");

    let legacy_ledger = Ledger::new("Legacy", LedgerBudgetPeriod::monthly());
    let mut legacy_path = storage.ledger_path("legacy-ledger");
    legacy_path.set_extension("json");
    fs::write(
        &legacy_path,
        to_string(&legacy_ledger).expect("serialize ledger"),
    )
    .expect("write legacy file");

    let loaded = storage
        .load_ledger("legacy-ledger")
        .expect("load legacy ledger");
    assert_eq!(loaded.name, legacy_ledger.name);
    let legacy_path = storage.ledger_path("legacy-ledger");
    let legacy_slug = legacy_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("slug");
    assert!(storage
        .list_ledgers()
        .unwrap()
        .contains(&legacy_slug.to_string()));
}
