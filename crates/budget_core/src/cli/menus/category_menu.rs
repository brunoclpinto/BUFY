use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::{state::MenuContextState, MenuError};

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let state = MenuContextState::capture(context);
    let menu = MenuUI::new("category menu", menu_items(&state)).with_context(Banner::text(context));
    renderer.show(&menu)
}

fn menu_items(state: &MenuContextState) -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("add", "add", "Add a category").with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("edit", "edit", "Edit a category").with_enabled(state.has_categories),
        MenuUIItem::new("remove", "remove", "Remove a category").with_enabled(state.has_categories),
        MenuUIItem::new("list", "list", "List categories").with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("show", "show", "Show category details").with_enabled(state.has_categories),
        MenuUIItem::new("budget", "budget", "Manage category budgets")
            .with_enabled(state.has_categories),
    ]
}
