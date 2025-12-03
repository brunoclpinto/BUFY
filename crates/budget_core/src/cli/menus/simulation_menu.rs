use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::{state::MenuContextState, MenuError};

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    let renderer = MenuRenderer::new();
    let state = MenuContextState::capture(context);
    let menu =
        MenuUI::new("simulation menu", menu_items(&state)).with_context(Banner::text(context));
    renderer.show(&menu)
}

fn menu_items(state: &MenuContextState) -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("new", "new", "Create a simulation").with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("enter", "enter", "Enter a simulation")
            .with_enabled(state.has_pending_simulations),
        MenuUIItem::new("leave", "leave", "Leave active simulation")
            .with_enabled(state.has_active_simulation),
        MenuUIItem::new("apply", "apply", "Apply simulation changes")
            .with_enabled(state.has_pending_simulations),
        MenuUIItem::new("discard", "discard", "Discard a simulation")
            .with_enabled(state.has_pending_simulations),
        MenuUIItem::new("list", "list", "List simulations").with_enabled(state.has_loaded_ledger),
        MenuUIItem::new("show", "show", "Show simulation details")
            .with_enabled(state.has_simulations),
    ]
}
