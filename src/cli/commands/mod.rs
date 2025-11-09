pub mod account;
pub mod category;
pub mod config;
pub mod ledger;
pub mod simulation;
pub mod system;
pub mod transaction;

use crate::cli::registry::{CommandEntry, CommandRegistry};

pub(crate) fn all_entries() -> Vec<CommandEntry> {
    let mut commands = Vec::new();
    commands.extend(system::definitions());
    commands.extend(ledger::definitions());
    commands.extend(config::definitions());
    commands.extend(account::definitions());
    commands.extend(category::definitions());
    commands.extend(transaction::definitions());
    commands.extend(simulation::definitions());
    commands
}

pub(crate) fn register_all(registry: &mut CommandRegistry) {
    for entry in all_entries() {
        registry.register(entry);
    }
}
