use super::main_menu::MenuError;
use super::sub_menu::{SubMenu, SubMenuItem};

pub fn show() -> Result<Option<&'static str>, MenuError> {
    let items = vec![
        SubMenuItem::new("accounts", "accounts", "List accounts"),
        SubMenuItem::new("categories", "categories", "List categories"),
        SubMenuItem::new("transactions", "transactions", "List transactions"),
        SubMenuItem::new("simulations", "simulations", "List simulations"),
        SubMenuItem::new("ledgers", "ledgers", "List ledgers"),
        SubMenuItem::new("backups", "backups", "List ledger backups"),
    ];
    SubMenu::new("list", items).show()
}
