use std::{cell::RefCell, collections::VecDeque, path::PathBuf};

use dialoguer::theme::ColorfulTheme;

use crate::{
    config::{Config, ConfigManager},
    core::ledger_manager::LedgerManager,
    ledger::{Ledger, Simulation},
    storage::json_backend::JsonStorage,
};

use super::registry::CommandRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    Interactive,
    Script,
}

#[derive(Default)]
pub struct SelectionOverride {
    queue: RefCell<VecDeque<Option<usize>>>,
}

impl SelectionOverride {
    #[cfg(test)]
    pub fn push(&self, choice: Option<usize>) {
        self.queue.borrow_mut().push_back(choice);
    }

    pub fn pop(&self) -> Option<Option<usize>> {
        self.queue.borrow_mut().pop_front()
    }

    pub fn has_choices(&self) -> bool {
        !self.queue.borrow().is_empty()
    }

    #[cfg(test)]
    pub fn clear(&self) {
        self.queue.borrow_mut().clear();
    }
}

pub struct ShellContext {
    pub mode: CliMode,
    pub registry: CommandRegistry,
    pub ledger_manager: LedgerManager,
    pub theme: ColorfulTheme,
    pub storage: JsonStorage,
    pub config_manager: ConfigManager,
    pub config: Config,
    pub ledger_path: Option<PathBuf>,
    pub active_simulation_name: Option<String>,
    pub selection_override: Option<SelectionOverride>,
    pub current_simulation: Option<Simulation>,
    pub last_command: Option<String>,
    pub running: bool,
}

impl ShellContext {
    pub fn current_ledger_opt(&self) -> Option<&Ledger> {
        self.ledger_manager.with_current(|ledger| ledger).ok()
    }

    pub fn current_ledger_mut_opt(&mut self) -> Option<&mut Ledger> {
        self.ledger_manager.with_current_mut(|ledger| ledger).ok()
    }

    pub fn is_simulation_active(&self) -> bool {
        self.current_simulation.is_some()
    }

    pub fn status(&self) -> String {
        format!(
            "ShellContext {{ running: {}, last_command: {:?}, simulation: {:?} }}",
            self.running,
            self.last_command,
            self.current_simulation
                .as_ref()
                .map(|sim| sim.name.as_str())
        )
    }
}
