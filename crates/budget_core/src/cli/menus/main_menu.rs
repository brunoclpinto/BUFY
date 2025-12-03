use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::{state::MenuContextState, MenuError};

const MAIN_MENU_HINT: &str = "(Use arrow keys to navigate, Enter to select, ESC to exit)";

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let banner = Banner::text(context);
    let state = MenuContextState::capture(context);
    let menu = MenuUI::new("Main menu", main_menu_items(&state))
        .with_context(banner)
        .with_footer_hint(MAIN_MENU_HINT);
    renderer.show(&menu)
}

fn main_menu_items(_state: &MenuContextState) -> Vec<MenuUIItem> {
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
        MenuUIItem::new("forecast", "forecast", "Forecast upcoming activity"),
        MenuUIItem::new(
            "list",
            "list",
            "List accounts, categories, transactions, simulations...",
        ),
        MenuUIItem::new("summary", "summary", "Show ledger summary"),
        MenuUIItem::new("config", "config", "Global CLI preferences"),
        MenuUIItem::new("help", "help", "Show available commands"),
        MenuUIItem::new("version", "version", "Show build metadata"),
        MenuUIItem::new("exit", "exit", "Exit the shell"),
    ]
}
