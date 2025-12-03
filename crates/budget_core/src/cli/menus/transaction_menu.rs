use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::{state::MenuContextState, MenuError};

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let state = MenuContextState::capture(context);
    let menu =
        MenuUI::new("transaction menu", menu_items(&state)).with_context(Banner::text(context));
    renderer.show(&menu)
}

fn menu_items(state: &MenuContextState) -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("add", "add", "Add a transaction")
            .with_enabled(state.has_loaded_ledger && state.has_accounts),
        MenuUIItem::new("edit", "edit", "Edit a transaction").with_enabled(state.has_transactions),
        MenuUIItem::new("remove", "remove", "Remove a transaction")
            .with_enabled(state.has_transactions),
        MenuUIItem::new("complete", "complete", "Mark a transaction as completed")
            .with_enabled(state.has_planned_transactions),
        MenuUIItem::new("list", "list", "List transactions").with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("show", "show", "Show transaction details")
            .with_enabled(state.has_transactions),
    ]
}
