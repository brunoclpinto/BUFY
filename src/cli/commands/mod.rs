pub mod account;
pub mod account_handlers;
pub mod backup;
pub mod category;
pub mod category_handlers;
pub mod config;
pub mod ledger;
pub mod ledger_handlers;
pub mod list_handlers;
pub mod recurring;
pub mod simulation;
pub mod simulation_handlers;
pub mod system;
pub mod transaction;
pub mod transaction_handlers;

use crate::cli::registry::{CommandEntry, CommandRegistry};

const ROOT_COMMAND_ORDER: &[&str] = &[
    "ledger",
    "account",
    "category",
    "transaction",
    "simulation",
    "list",
    "summary",
    "forecast",
    "config",
    "help",
    "version",
    "exit",
];

pub(crate) fn all_entries() -> Vec<CommandEntry> {
    let mut commands = Vec::new();
    commands.extend(ledger::definitions());
    commands.extend(account::definitions());
    commands.extend(category::definitions());
    commands.extend(transaction::definitions());
    commands.extend(simulation::definitions());
    commands.extend(config::definitions());
    commands.extend(system::definitions());
    commands
}

pub(crate) fn register_all(registry: &mut CommandRegistry) {
    let mut entries = all_entries();
    entries.sort_by_key(|entry| {
        ROOT_COMMAND_ORDER
            .iter()
            .position(|name| entry.name.eq_ignore_ascii_case(name))
            .unwrap_or(ROOT_COMMAND_ORDER.len())
    });
    for entry in entries {
        registry.register(entry);
    }
}
