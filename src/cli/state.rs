use std::path::PathBuf;

use crate::ledger::Ledger;

/// Shared CLI runtime state.
///
/// Holds the currently loaded ledger, associated path/name, and
/// the active simulation identifier (if any). All command handlers
/// should read/write through this struct.
#[derive(Default)]
pub struct CliState {
    pub ledger: Option<Ledger>,
    pub ledger_path: Option<PathBuf>,
    pub ledger_name: Option<String>,
    pub active_simulation: Option<String>,
}

impl CliState {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.ledger = None;
        self.ledger_path = None;
        self.ledger_name = None;
        self.active_simulation = None;
    }

    pub fn set_ledger(&mut self, ledger: Ledger, path: Option<PathBuf>, name: Option<String>) {
        self.ledger = Some(ledger);
        self.ledger_path = path;
        self.ledger_name = name;
        self.active_simulation = None;
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.ledger_path = path;
        if self.ledger_path.is_some() {
            self.ledger_name = None;
        }
    }

    pub fn set_named(&mut self, name: Option<String>) {
        self.ledger_name = name;
    }

    pub fn ledger_name(&self) -> Option<&str> {
        self.ledger_name.as_deref()
    }

    pub fn set_active_simulation(&mut self, name: Option<String>) {
        self.active_simulation = name;
    }

    pub fn active_simulation(&self) -> Option<&str> {
        self.active_simulation.as_deref()
    }

    pub fn ledger_ref(&self) -> Option<&Ledger> {
        self.ledger.as_ref()
    }

    pub fn ledger_mut_ref(&mut self) -> Option<&mut Ledger> {
        self.ledger.as_mut()
    }
}
