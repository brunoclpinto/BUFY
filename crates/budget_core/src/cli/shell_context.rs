//! Shared runtime state for CLI interactions and command execution.

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use dialoguer::theme::ColorfulTheme;

use crate::{
    config::{Config, ConfigManager},
    core::ledger_manager::LedgerManager,
    ledger::Simulation,
};
use bufy_core::Clock;
use bufy_storage_json::JsonLedgerStorage as JsonStorage;

use super::{formatters::CliFormatters, registry::CommandRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    Interactive,
    Script,
}

pub struct ShellContext {
    pub mode: CliMode,
    pub registry: CommandRegistry,
    pub ledger_manager: Arc<RwLock<LedgerManager>>,
    pub theme: ColorfulTheme,
    pub storage: JsonStorage,
    pub clock: Arc<dyn Clock>,
    pub formatters: CliFormatters,
    pub config_manager: Arc<RwLock<ConfigManager>>,
    pub config: Arc<RwLock<Config>>,
    pub ledger_path: Option<PathBuf>,
    pub active_simulation_name: Option<String>,
    pub current_simulation: Option<Simulation>,
    pub last_command: Option<String>,
    pub running: bool,
}

impl ShellContext {
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
