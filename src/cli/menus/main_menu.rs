use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

const MAIN_MENU_HINT: &str =
    "(Use arrow keys to navigate, Enter to select, ESC to exit)";

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let banner = Banner::text(context);
    let title = format!("Main menu - {}", format_status_label(&banner));
    let menu = MenuUI::new(title, main_menu_items())
        .with_context(banner)
        .with_footer_hint(MAIN_MENU_HINT);
    renderer.show(&menu)
}

fn main_menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new(
            "ledger",
            "ledger",
            "Ledger operations (new, load, save, backup, restore...)",
        ),
        MenuUIItem::new("account", "account", "Manage accounts via wizard flows"),
        MenuUIItem::new("category", "category", "Manage categories and budgets"),
        MenuUIItem::new(
            "transaction",
            "transaction",
            "Manage transactions via wizard flows",
        ),
        MenuUIItem::new(
            "simulation",
            "simulation",
            "Manage simulations and what-if scenarios",
        ),
        MenuUIItem::new(
            "list",
            "list",
            "List accounts, categories, transactions, simulations...",
        ),
        MenuUIItem::new("summary", "summary", "Show ledger summary"),
        MenuUIItem::new("forecast", "forecast", "Forecast upcoming activity"),
        MenuUIItem::new("config", "config", "Global CLI preferences"),
        MenuUIItem::new("help", "help", "Show available commands"),
        MenuUIItem::new("version", "version", "Show build metadata"),
        MenuUIItem::new("exit", "exit", "Exit the shell"),
    ]
}

fn format_status_label(raw_status: &str) -> String {
    let stripped = raw_status.trim_end_matches(" ⮞").trim();
    if stripped.eq_ignore_ascii_case("no-ledger") {
        "No Ledger".to_string()
    } else {
        capitalize(stripped)
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
