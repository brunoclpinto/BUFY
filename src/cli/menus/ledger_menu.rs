use super::main_menu::MenuError;
use super::sub_menu::{SubMenu, SubMenuItem};

pub fn show() -> Result<Option<&'static str>, MenuError> {
    let items = vec![
        SubMenuItem::new("new", "new", "Create a new ledger"),
        SubMenuItem::new("load", "load", "Load an existing ledger"),
        SubMenuItem::new("save", "save", "Save current ledger"),
        SubMenuItem::new("backup", "backup", "Create a snapshot"),
        SubMenuItem::new("restore", "restore", "Restore from snapshot"),
        SubMenuItem::new("list", "list", "List ledgers and backups"),
        SubMenuItem::new("delete", "delete", "Delete a ledger"),
    ];
    SubMenu::new("ledger", items).show()
}
