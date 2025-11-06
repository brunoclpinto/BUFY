use std::collections::HashMap;

pub mod account;
pub mod category;
pub mod config;
pub mod ledger;
pub mod simulation;
pub mod system;
pub mod transaction;

use crate::cli::core::{CommandResult, ShellContext};

pub(crate) fn all_definitions() -> Vec<CommandDefinition> {
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

pub type CommandHandler = fn(&mut ShellContext, &[&str]) -> CommandResult;

#[derive(Clone)]
pub struct CommandDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
    pub handler: CommandHandler,
}

impl CommandDefinition {
    pub const fn new(
        name: &'static str,
        description: &'static str,
        usage: &'static str,
        handler: CommandHandler,
    ) -> Self {
        Self {
            name,
            description,
            usage,
            handler,
        }
    }
}

pub struct CommandRegistry {
    commands: HashMap<&'static str, CommandDefinition>,
    order: Vec<&'static str>,
}

impl CommandRegistry {
    pub fn new(definitions: Vec<CommandDefinition>) -> Self {
        let mut commands = HashMap::new();
        let mut order = Vec::new();
        for definition in definitions {
            order.push(definition.name);
            commands.insert(definition.name, definition);
        }
        Self { commands, order }
    }

    pub fn get(&self, name: &str) -> Option<&CommandDefinition> {
        self.commands.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &CommandDefinition> {
        self.order
            .iter()
            .filter_map(move |name| self.commands.get(name))
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.order.iter().copied()
    }
}
