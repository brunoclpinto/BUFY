use std::path::PathBuf;

use crate::{
    cli::selectors::{SelectionItem, SelectionProvider},
    ledger::{Account, Category, Ledger, Simulation, Transaction},
    utils::persistence::{BackupInfo, LedgerStore},
};
use chrono::Local;

use crate::cli::state::CliState;

#[derive(Debug)]
pub enum ProviderError {
    MissingLedger,
    Store(String),
}

impl From<std::io::Error> for ProviderError {
    fn from(err: std::io::Error) -> Self {
        ProviderError::Store(err.to_string())
    }
}

pub struct AccountSelectionProvider<'a> {
    state: &'a CliState,
}

impl<'a> AccountSelectionProvider<'a> {
    pub fn new(state: &'a CliState) -> Self {
        Self { state }
    }
}

impl<'a> SelectionProvider for AccountSelectionProvider<'a> {
    type Id = usize;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger
            .accounts
            .iter()
            .enumerate()
            .map(|(idx, account)| account_item(idx, account))
            .collect())
    }
}

pub struct CategorySelectionProvider<'a> {
    state: &'a CliState,
}

impl<'a> CategorySelectionProvider<'a> {
    pub fn new(state: &'a CliState) -> Self {
        Self { state }
    }
}

impl<'a> SelectionProvider for CategorySelectionProvider<'a> {
    type Id = usize;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger
            .categories
            .iter()
            .enumerate()
            .map(|(idx, category)| category_item(idx, category))
            .collect())
    }
}

pub struct TransactionSelectionProvider<'a> {
    state: &'a CliState,
}

impl<'a> TransactionSelectionProvider<'a> {
    pub fn new(state: &'a CliState) -> Self {
        Self { state }
    }
}

impl<'a> SelectionProvider for TransactionSelectionProvider<'a> {
    type Id = usize;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger
            .transactions
            .iter()
            .enumerate()
            .map(|(idx, txn)| transaction_item(idx, txn, ledger))
            .collect())
    }
}

pub struct SimulationSelectionProvider<'a> {
    state: &'a CliState,
}

impl<'a> SimulationSelectionProvider<'a> {
    pub fn new(state: &'a CliState) -> Self {
        Self { state }
    }
}

impl<'a> SelectionProvider for SimulationSelectionProvider<'a> {
    type Id = String;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger.simulations().iter().map(simulation_item).collect())
    }
}

pub struct LedgerBackupSelectionProvider<'a> {
    state: &'a CliState,
    store: &'a LedgerStore,
}

impl<'a> LedgerBackupSelectionProvider<'a> {
    pub fn new(state: &'a CliState, store: &'a LedgerStore) -> Self {
        Self { state, store }
    }
}

impl<'a> SelectionProvider for LedgerBackupSelectionProvider<'a> {
    type Id = PathBuf;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let name = self
            .state
            .ledger_name()
            .ok_or(ProviderError::MissingLedger)?;
        let backups = self
            .store
            .list_backups(name)
            .map_err(|err| ProviderError::Store(err.to_string()))?;
        Ok(backups.into_iter().map(backup_item).collect())
    }
}

pub struct ConfigBackupSelectionProvider<'a> {
    store: &'a LedgerStore,
}

impl<'a> ConfigBackupSelectionProvider<'a> {
    pub fn new(store: &'a LedgerStore) -> Self {
        Self { store }
    }
}

impl<'a> SelectionProvider for ConfigBackupSelectionProvider<'a> {
    type Id = PathBuf;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let backups = self
            .store
            .list_config_backups()
            .map_err(|err| ProviderError::Store(err.to_string()))?;
        Ok(backups
            .into_iter()
            .map(|info| {
                let file_name = info
                    .path
                    .file_name()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("config-backup");
                let created = info.created_at.with_timezone(&Local);
                let mut label = format!(
                    "{:<30} (Created: {})",
                    file_name,
                    created.format("%Y-%m-%d %H:%M")
                );
                if let Some(note) = info
                    .note
                    .as_ref()
                    .map(|n| n.trim())
                    .filter(|n| !n.is_empty())
                {
                    label.push_str(&format!("  [note: {}]", note));
                }
                SelectionItem::new(info.path, label)
            })
            .collect())
    }
}

fn account_item(index: usize, account: &Account) -> SelectionItem<usize> {
    let mut subtitle = format!("{:?}", account.kind);
    if let Some(balance) = account.opening_balance {
        subtitle.push_str(&format!(" • balance {:.2}", balance));
    }
    SelectionItem::new(index, account.name.clone()).with_subtitle(subtitle)
}

fn category_item(index: usize, category: &Category) -> SelectionItem<usize> {
    let mut item = SelectionItem::new(index, category.name.clone())
        .with_subtitle(format!("{:?}", category.kind));
    if let Some(parent) = category.parent_id {
        item = item.with_category(format!("parent: {}", parent));
    }
    item
}

fn transaction_item(index: usize, txn: &Transaction, ledger: &Ledger) -> SelectionItem<usize> {
    let from = ledger
        .account(txn.from_account)
        .map(|acct| acct.name.as_str())
        .unwrap_or("Unknown");
    let to = ledger
        .account(txn.to_account)
        .map(|acct| acct.name.as_str())
        .unwrap_or("Unknown");
    let category = txn
        .category_id
        .and_then(|id| ledger.category(id))
        .map(|cat| cat.name.as_str())
        .unwrap_or("Uncategorized");
    let amount = txn.actual_amount.unwrap_or(txn.budgeted_amount);
    let status = format!("{:?}", txn.status);
    let label = format!(
        "[{:>3}] {} | {} -> {} | {:.2} | {} | {}",
        index, txn.scheduled_date, from, to, amount, category, status
    );
    SelectionItem::new(index, label)
}

fn simulation_item(sim: &Simulation) -> SelectionItem<String> {
    let created = sim.created_at.with_timezone(&Local);
    let mut label = format!("{:<30} (Created: {})", sim.name, created.format("%Y-%m-%d"));
    if let Some(note) = &sim.notes {
        let trimmed = note.trim();
        if !trimmed.is_empty() {
            label.push_str(&format!("  [note: {}]", trimmed));
        }
    }
    label.push_str(&format!("  [status: {:?}]", sim.status));
    SelectionItem::new(sim.name.clone(), label)
}

fn backup_item(info: BackupInfo) -> SelectionItem<PathBuf> {
    let created = info.timestamp.with_timezone(&Local);
    let file_name = info
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("backup");
    let label = format!(
        "{:<30} (Created: {})",
        file_name,
        created.format("%Y-%m-%d %H:%M")
    );
    SelectionItem::new(info.path.clone(), label)
}
