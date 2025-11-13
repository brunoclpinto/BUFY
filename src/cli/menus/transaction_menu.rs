use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show() -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("transaction menu", menu_items());
    renderer.show(&menu)
}

fn menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("add", "add", "Add a transaction"),
        MenuUIItem::new("edit", "edit", "Edit a transaction"),
        MenuUIItem::new("remove", "remove", "Remove a transaction"),
        MenuUIItem::new("complete", "complete", "Mark a transaction as completed"),
        MenuUIItem::new("list", "list", "List transactions"),
        MenuUIItem::new("show", "show", "Show transaction details"),
    ]
}
