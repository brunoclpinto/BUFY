use std::collections::HashMap;

use super::{CliApp, CommandResult};

pub type CommandHandler = fn(&mut CliApp, &[&str]) -> CommandResult;

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

    #[allow(dead_code)]
    pub fn execute(&self, app: &mut CliApp, args: &[&str]) -> CommandResult {
        (self.handler)(app, args)
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

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = &CommandDefinition> {
        self.order
            .iter()
            .filter_map(move |name| self.commands.get(name))
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.order.iter().copied()
    }
}
