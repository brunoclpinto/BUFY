use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::{state::MenuContextState, MenuError};

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let state = MenuContextState::capture(context);
    let menu = MenuUI::new("list menu", menu_items(&state)).with_context(Banner::text(context));
    renderer.show(&menu)
}

fn menu_items(state: &MenuContextState) -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("accounts", "accounts", "List accounts")
            .with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("categories", "categories", "List categories")
            .with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("transactions", "transactions", "List transactions")
            .with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("simulations", "simulations", "List simulations")
            .with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("ledgers", "ledgers", "List ledgers"),
        MenuUIItem::new("backups", "backups", "List ledger backups")
            .with_enabled(state.has_named_ledger),
    ]
}
