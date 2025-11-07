use std::path::PathBuf;

use crate::{core::ledger_manager::LedgerManager, ledger::Ledger};

/// Shared CLI runtime state.
///
/// Holds the active ledger manager reference along with interactive metadata.
pub struct CliState {
    ledger_manager: LedgerManager,
    pub active_simulation: Option<String>,
}

impl CliState {
    pub fn new(ledger_manager: LedgerManager) -> Self {
        Self {
            ledger_manager,
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
        self.active_simulation = None;
    }

    pub fn set_ledger(&mut self, ledger: Ledger, path: Option<PathBuf>, name: Option<String>) {
        self.ledger_manager.set_current(ledger, path, name);
        self.active_simulation = None;
    }

    pub fn ledger_name(&self) -> Option<&str> {
        self.ledger_manager.current_name()
    }

    pub fn ledger_path(&self) -> Option<PathBuf> {
        self.ledger_manager
            .current_path()
            .map(|path| path.to_path_buf())
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
