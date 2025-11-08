use crate::{
    cli::selectors::{SelectionItem, SelectionProvider},
    config::ConfigManager,
    core::ledger_manager::LedgerManager,
    ledger::{Account, Category, Ledger, Simulation, Transaction},
};
use chrono::{DateTime, Local, NaiveDateTime, Utc};

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
    manager: &'a LedgerManager,
}

impl<'a> LedgerBackupSelectionProvider<'a> {
    pub fn new(state: &'a CliState, manager: &'a LedgerManager) -> Self {
        Self { state, manager }
    }
}

impl<'a> SelectionProvider for LedgerBackupSelectionProvider<'a> {
    type Id = String;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let name = self
            .state
            .ledger_name()
            .ok_or(ProviderError::MissingLedger)?;
        let backups = self
            .manager
            .list_backups(name)
            .map_err(|err| ProviderError::Store(err.to_string()))?;
        Ok(backups
            .into_iter()
            .map(|item| SelectionItem::new(item.clone(), backup_label(&item)))
            .collect())
    }
}

pub struct ConfigBackupSelectionProvider<'a> {
    manager: &'a ConfigManager,
}

impl<'a> ConfigBackupSelectionProvider<'a> {
    pub fn new(manager: &'a ConfigManager) -> Self {
        Self { manager }
    }
}

impl<'a> SelectionProvider for ConfigBackupSelectionProvider<'a> {
    type Id = String;
    type Error = ProviderError;

    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error> {
        let backups = self
            .manager
            .list_backups()
            .map_err(|err| ProviderError::Store(err.to_string()))?;
        Ok(backups
            .into_iter()
            .map(|name| SelectionItem::new(name.clone(), backup_label(&name)))
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

fn backup_label(name: &str) -> String {
    let trimmed = name.trim_end_matches(".json");
    let segments: Vec<&str> = trimmed.split('_').collect();
    if segments.len() < 3 {
        return name.to_string();
    }
    let date_part = segments[segments.len() - 2];
    let time_part = segments[segments.len() - 1];
    if date_part.len() == 8
        && time_part.len() == 4
        && date_part.chars().all(|c| c.is_ascii_digit())
        && time_part.chars().all(|c| c.is_ascii_digit())
    {
        if let Ok(naive) =
            NaiveDateTime::parse_from_str(&format!("{}{}", date_part, time_part), "%Y%m%d%H%M")
        {
            let utc = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
            let local = utc.with_timezone(&Local);
            return format!("{:<30} (Created: {})", name, local.format("%Y-%m-%d %H:%M"));
        }
    }
    name.to_string()
}
