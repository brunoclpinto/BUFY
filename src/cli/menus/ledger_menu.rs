use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show() -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("ledger menu", menu_items());
    renderer.show(&menu)
}

fn menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("new", "new", "Create a new ledger"),
        MenuUIItem::new("load", "load", "Load an existing ledger"),
        MenuUIItem::new("save", "save", "Save current ledger"),
        MenuUIItem::new("backup", "backup", "Create a snapshot"),
        MenuUIItem::new("restore", "restore", "Restore from snapshot"),
        MenuUIItem::new("list", "list", "List ledgers and backups"),
        MenuUIItem::new("delete", "delete", "Delete a ledger"),
    ]
}
