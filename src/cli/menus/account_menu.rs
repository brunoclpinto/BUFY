use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show() -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("account menu", menu_items());
    renderer.show(&menu)
}

fn menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("add", "add", "Add an account"),
        MenuUIItem::new("edit", "edit", "Edit an account"),
        MenuUIItem::new("remove", "remove", "Remove an account"),
        MenuUIItem::new("list", "list", "List accounts"),
        MenuUIItem::new("show", "show", "Show account details"),
    ]
}
