use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use bufy_domain::Ledger;

use crate::CoreError;

/// Describes a persisted backup artifact for a ledger.
#[derive(Debug, Clone)]
pub struct LedgerBackupInfo {
    pub ledger: String,
    pub id: String,
    pub created_at: String,
    pub path: PathBuf,
}

/// Abstraction over persistence backends capable of storing ledgers and backups.
pub trait LedgerStorage: Send + Sync {
    fn save_ledger(&self, name: &str, ledger: &Ledger) -> Result<(), CoreError>;
    fn load_ledger(&self, name: &str) -> Result<Ledger, CoreError>;
    fn list_ledgers(&self) -> Result<Vec<String>, CoreError>;
    fn delete_ledger(&self, name: &str) -> Result<(), CoreError>;
    fn save_ledger_to_path(&self, ledger: &Ledger, path: &Path) -> Result<(), CoreError>;
    fn load_ledger_from_path(&self, path: &Path) -> Result<Ledger, CoreError>;
    fn backup_ledger(
        &self,
        name: &str,
        ledger: &Ledger,
        note: Option<&str>,
    ) -> Result<LedgerBackupInfo, CoreError>;
    fn list_backups(&self, name: &str) -> Result<Vec<LedgerBackupInfo>, CoreError>;
    fn restore_backup(&self, backup: &LedgerBackupInfo) -> Result<Ledger, CoreError>;
}

/// Detects dangling references and other anomalies within a ledger snapshot.
pub fn ledger_warnings(ledger: &Ledger) -> Vec<String> {
    let account_ids: HashSet<_> = ledger.accounts.iter().map(|a| a.id).collect();
    let category_ids: HashSet<_> = ledger.categories.iter().map(|c| c.id).collect();
    let mut warnings = Vec::new();

    for txn in &ledger.transactions {
        if !account_ids.contains(&txn.from_account) {
            warnings.push(format!(
                "transaction {} references unknown from_account {}",
                txn.id, txn.from_account
            ));
        }
        if !account_ids.contains(&txn.to_account) {
            warnings.push(format!(
                "transaction {} references unknown to_account {}",
                txn.id, txn.to_account
            ));
        }
        if let Some(category) = txn.category_id {
            if !category_ids.contains(&category) {
                warnings.push(format!(
                    "transaction {} references missing category {}",
                    txn.id, category
                ));
            }
        }
        if let Some(rule) = txn.recurrence.as_ref() {
            if !rule.is_active() && rule.next_scheduled.is_none() {
                warnings.push(format!(
                    "recurrence {} inactive with no next date",
                    rule.series_id
                ));
            }
        }
    }
    warnings
}
