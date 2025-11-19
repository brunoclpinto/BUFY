use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::{state::MenuContextState, MenuError};

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let state = MenuContextState::capture(context);
    let menu = MenuUI::new("ledger menu", menu_items(&state)).with_context(Banner::text(context));
    renderer.show(&menu)
}

fn menu_items(state: &MenuContextState) -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("new", "new", "Create a new ledger"),
        MenuUIItem::new("load", "load", "Load an existing ledger"),
        MenuUIItem::new("save", "save", "Save current ledger")
            .with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("backup", "backup", "Create a snapshot")
            .with_enabled(state.has_named_ledger),
        MenuUIItem::new("restore", "restore", "Restore from snapshot")
            .with_enabled(state.has_named_ledger),
        MenuUIItem::new("list", "list", "List ledgers and backups"),
        MenuUIItem::new("delete", "delete", "Delete a ledger"),
    ]
}
