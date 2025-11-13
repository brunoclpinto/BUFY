use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show() -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("category menu", menu_items());
    renderer.show(&menu)
}

fn menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("add", "add", "Add a category"),
        MenuUIItem::new("edit", "edit", "Edit a category"),
        MenuUIItem::new("remove", "remove", "Remove a category"),
        MenuUIItem::new("list", "list", "List categories"),
        MenuUIItem::new("show", "show", "Show category details"),
        MenuUIItem::new("budget", "budget", "Manage category budgets"),
    ]
}
