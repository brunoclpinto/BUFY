use super::main_menu::MenuError;
use super::sub_menu::{SubMenu, SubMenuItem};

pub fn show() -> Result<Option<&'static str>, MenuError> {
    let items = vec![
        SubMenuItem::new("add", "add", "Add an account"),
        SubMenuItem::new("edit", "edit", "Edit an account"),
        SubMenuItem::new("remove", "remove", "Remove an account"),
        SubMenuItem::new("list", "list", "List accounts"),
        SubMenuItem::new("show", "show", "Show account details"),
    ];
    SubMenu::new("account", items).show()
}
