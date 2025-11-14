use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("list menu", menu_items())
        .with_context(Banner::text(context));
    renderer.show(&menu)
}

fn menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("accounts", "accounts", "List accounts"),
        MenuUIItem::new("categories", "categories", "List categories"),
        MenuUIItem::new("transactions", "transactions", "List transactions"),
        MenuUIItem::new("simulations", "simulations", "List simulations"),
        MenuUIItem::new("ledgers", "ledgers", "List ledgers"),
        MenuUIItem::new("backups", "backups", "List ledger backups"),
    ]
}
