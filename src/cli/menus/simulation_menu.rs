use crate::cli::core::ShellContext;
use crate::cli::ui::banner::Banner;
use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};

use super::MenuError;

pub fn show(context: &ShellContext) -> Result<Option<String>, MenuError> {
    Banner::render(context);
    let renderer = MenuRenderer::new();
    let menu = MenuUI::new("simulation menu", menu_items());
    renderer.show(&menu)
}

fn menu_items() -> Vec<MenuUIItem> {
    vec![
        MenuUIItem::new("new", "new", "Create a simulation"),
        MenuUIItem::new("enter", "enter", "Enter a simulation"),
        MenuUIItem::new("leave", "leave", "Leave active simulation"),
        MenuUIItem::new("apply", "apply", "Apply simulation changes"),
        MenuUIItem::new("discard", "discard", "Discard a simulation"),
        MenuUIItem::new("list", "list", "List simulations"),
        MenuUIItem::new("show", "show", "Show simulation details"),
    ]
}
