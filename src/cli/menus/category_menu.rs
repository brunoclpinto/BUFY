use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("category menu", menu_items())
        .with_context(Banner::text(context));
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
