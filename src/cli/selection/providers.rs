use std::fs;
use std::path::PathBuf;

use uuid::Uuid;

use crate::{
    cli::selectors::{SelectionItem, SelectionProvider},
    ledger::{Account, Category, Simulation, Transaction},
    utils::persistence::{BackupInfo, LedgerStore},
};

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
    type Id = Uuid;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger.accounts.iter().map(account_item).collect())
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
    type Id = Uuid;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger.categories.iter().map(category_item).collect())
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
    type Id = Uuid;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let ledger = self
            .state
            .ledger_ref()
            .ok_or(ProviderError::MissingLedger)?;
        Ok(ledger.transactions.iter().map(transaction_item).collect())
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
        let dir = self.store.base_dir().join("state-backups");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                let label = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("config-backup")
                    .to_string();
                entries.push(SelectionItem::new(path.clone(), label).with_category("config"));
            }
        }
        Ok(entries)
    }
}

fn account_item(account: &Account) -> SelectionItem<Uuid> {
    SelectionItem::new(account.id, account.name.clone())
        .with_subtitle(format!("{:?}", account.kind))
}

fn category_item(category: &Category) -> SelectionItem<Uuid> {
    let mut item = SelectionItem::new(category.id, category.name.clone())
        .with_subtitle(format!("{:?}", category.kind));
    if let Some(parent) = category.parent_id {
        item = item.with_category(format!("parent: {}", parent));
    }
    item
}

fn transaction_item(txn: &Transaction) -> SelectionItem<Uuid> {
    let label = txn
        .recurrence
        .as_ref()
        .map(|_| format!("{} • recurring", txn.scheduled_date))
        .unwrap_or_else(|| txn.scheduled_date.to_string());
    let amount = txn.actual_amount.unwrap_or(txn.budgeted_amount);
    SelectionItem::new(txn.id, label).with_subtitle(format!("{:.2}", amount))
}

fn simulation_item(sim: &Simulation) -> SelectionItem<String> {
    let subtitle = format!("status: {:?}", sim.status);
    SelectionItem::new(sim.name.clone(), sim.name.clone()).with_subtitle(subtitle)
}

fn backup_item(info: BackupInfo) -> SelectionItem<PathBuf> {
    SelectionItem::new(info.path.clone(), info.timestamp.to_string())
}
