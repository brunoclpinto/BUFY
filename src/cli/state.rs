use std::path::PathBuf;

use crate::{core::ledger_manager::LedgerManager, ledger::Ledger};

/// Shared CLI runtime state.
///
/// Holds the active ledger manager reference along with interactive metadata.
pub struct CliState {
    ledger_manager: LedgerManager,
    ledger_path: Option<PathBuf>,
    pub active_simulation: Option<String>,
}

impl CliState {
    pub fn new(ledger_manager: LedgerManager) -> Self {
        Self {
            ledger_manager,
            ledger_path: None,
            active_simulation: None,
        }
    }

    pub fn manager(&self) -> &LedgerManager {
        &self.ledger_manager
    }

    pub fn manager_mut(&mut self) -> &mut LedgerManager {
        &mut self.ledger_manager
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.ledger_manager.clear();
        self.ledger_path = None;
        self.active_simulation = None;
    }

    pub fn set_ledger(&mut self, ledger: Ledger, path: Option<PathBuf>, name: Option<String>) {
        self.ledger_manager.set_current(ledger, path.clone(), name);
        self.ledger_path = path;
        self.active_simulation = None;
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.ledger_path = path;
    }

    pub fn ledger_name(&self) -> Option<&str> {
        self.ledger_manager.current_name()
    }

    pub fn ledger_path(&self) -> Option<PathBuf> {
        self.ledger_path.clone()
    }

    pub fn set_active_simulation(&mut self, name: Option<String>) {
        self.active_simulation = name;
    }

    pub fn active_simulation(&self) -> Option<&str> {
        self.active_simulation.as_deref()
    }

    pub fn ledger_ref(&self) -> Option<&Ledger> {
        self.ledger_manager.current.as_ref()
    }

    pub fn ledger_mut_ref(&mut self) -> Option<&mut Ledger> {
        self.ledger_manager.current.as_mut()
    }
}
