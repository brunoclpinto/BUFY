use super::main_menu::MenuError;
use super::sub_menu::{SubMenu, SubMenuItem};

pub fn show() -> Result<Option<&'static str>, MenuError> {
    let items = vec![
        SubMenuItem::new("add", "add", "Add a transaction"),
        SubMenuItem::new("edit", "edit", "Edit a transaction"),
        SubMenuItem::new("remove", "remove", "Remove a transaction"),
        SubMenuItem::new("complete", "complete", "Mark a transaction as completed"),
        SubMenuItem::new("list", "list", "List transactions"),
        SubMenuItem::new("show", "show", "Show transaction details"),
    ];
    SubMenu::new("transaction", items).show()
}
