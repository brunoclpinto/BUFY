use crate::cli::io;
use crate::cli::output::section as output_section;
use crate::cli::registry::{CommandEntry, CommandRegistry};

pub fn print_overview(registry: &CommandRegistry) {
    output_section("Available commands");
    for entry in registry.list() {
        io::print_info(format!("  {:<16} {}", entry.name, entry.description));
    }
    io::print_info("Use `help <command>` for details.");
    io::print_info("Use arrows or type command names; press Enter to execute.");
}

pub fn print_command(entry: &CommandEntry) {
    output_section(format!("Help: {}", entry.name));
    io::print_info(format!("  Description: {}", entry.description));
    io::print_info(format!("  Usage: {}", entry.usage));
    io::print_info("Use arrows or type command names; press Enter to execute.");
}
