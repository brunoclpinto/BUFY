use crate::cli::registry::{CommandEntry, CommandRegistry};
use crate::cli::ui::formatting::Formatter;

pub fn print_overview(registry: &CommandRegistry) {
    let formatter = Formatter::new();
    formatter.print_header("Available commands");
    let rows: Vec<_> = registry
        .list()
        .into_iter()
        .map(|entry| (entry.name, entry.description))
        .collect();
    formatter.print_two_column(&rows);
    formatter.print_detail("Use `help <command>` for details.");
    formatter.print_detail(formatter.navigation_hint());
}

pub fn print_command(entry: &CommandEntry) {
    let formatter = Formatter::new();
    formatter.print_header(format!("Help: {}", entry.name));
    formatter.print_detail(format!("Description: {}", entry.description));
    formatter.print_detail(format!("Usage: {}", entry.usage));
    formatter.print_detail(formatter.navigation_hint());
}
