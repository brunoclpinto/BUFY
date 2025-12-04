use crate::cli::registry::{CommandEntry, CommandRegistry};
use crate::cli::ui::formatting::Formatter;
use crate::cli::ui::style::UiStyle;
use crate::cli::ui::{DetailField, DetailViewRenderer, Menu, MenuRenderer};

pub fn print_overview(registry: &CommandRegistry, style: &UiStyle) {
    let mut menu = Menu::new("Available commands");
    for entry in registry.list() {
        menu.add_item(entry.name, Some(entry.description), true);
    }
    MenuRenderer::render(&menu, style);
    let formatter = Formatter::new();
    formatter.print_detail("Use `help <command>` for details.");
    formatter.print_detail(formatter.navigation_hint());
}

pub fn print_command(entry: &CommandEntry, style: &UiStyle) {
    let fields = vec![
        DetailField::new("description", format!("\"{}\"", entry.description)),
        DetailField::new("usage", format!("\"{}\"", entry.usage)),
        DetailField::new("alias", format!("\"{}\"", entry.name)),
    ];
    DetailViewRenderer::render(format!("Help: {}", entry.name), &fields, style);
    let formatter = Formatter::new();
    formatter.print_detail(formatter.navigation_hint());
}
