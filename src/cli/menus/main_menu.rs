use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show(context_banner: &str) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("Main menu", main_menu_items()).with_context(context_banner);
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
