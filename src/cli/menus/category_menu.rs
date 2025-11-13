use super::main_menu::MenuError;
use super::sub_menu::{SubMenu, SubMenuItem};

pub fn show() -> Result<Option<&'static str>, MenuError> {
    let items = vec![
        SubMenuItem::new("add", "add", "Add a category"),
        SubMenuItem::new("edit", "edit", "Edit a category"),
        SubMenuItem::new("remove", "remove", "Remove a category"),
        SubMenuItem::new("list", "list", "List categories"),
        SubMenuItem::new("show", "show", "Show category details"),
        SubMenuItem::new("budget", "budget", "Manage category budgets"),
    ];
    SubMenu::new("category", items).show()
}
