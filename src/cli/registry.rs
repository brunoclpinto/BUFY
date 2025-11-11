use std::collections::HashMap;

use crate::cli::core::CommandResult;
use crate::cli::shell_context::ShellContext;

pub type CommandHandler = fn(&mut ShellContext, &[&str]) -> CommandResult;

pub struct CommandEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
    pub handler: CommandHandler,
}

impl CommandEntry {
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
    commands: HashMap<&'static str, CommandEntry>,
    order: Vec<&'static str>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn register(&mut self, entry: CommandEntry) {
        let name = entry.name;
        if self.commands.insert(name, entry).is_none() {
            self.order.push(name);
        }
    }

    pub fn get(&self, name: &str) -> Option<&CommandEntry> {
        self.commands.get(name)
    }

    pub fn list(&self) -> Vec<&CommandEntry> {
        self.order
            .iter()
            .filter_map(|name| self.commands.get(name))
            .collect()
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.order.iter().copied()
    }

    pub fn handler(&self, name: &str) -> Option<CommandHandler> {
        self.commands.get(name).map(|entry| entry.handler)
    }
}
