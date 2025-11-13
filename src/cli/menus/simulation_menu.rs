use super::main_menu::MenuError;
use super::sub_menu::{SubMenu, SubMenuItem};

pub fn show() -> Result<Option<&'static str>, MenuError> {
    let items = vec![
        SubMenuItem::new("new", "new", "Create a simulation"),
        SubMenuItem::new("enter", "enter", "Enter a simulation"),
        SubMenuItem::new("leave", "leave", "Leave active simulation"),
        SubMenuItem::new("apply", "apply", "Apply simulation changes"),
        SubMenuItem::new("discard", "discard", "Discard a simulation"),
        SubMenuItem::new("list", "list", "List simulations"),
        SubMenuItem::new("show", "show", "Show simulation details"),
    ];
    SubMenu::new("simulation", items).show()
}
