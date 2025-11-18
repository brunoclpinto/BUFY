use crate::cli::commands::account::list_accounts;
use crate::cli::commands::backup::list_backups;
use crate::cli::commands::category::list_categories;
use crate::cli::commands::ledger::list_ledgers;
use crate::cli::commands::recurring::list_recurring;
use crate::cli::commands::simulation::list_simulations;
use crate::cli::commands::transaction::list_transactions;
use crate::cli::core::{CommandError, CommandResult, ShellContext};

pub fn dispatch(context: &mut ShellContext, key: &str) -> CommandResult {
    match key.to_ascii_lowercase().as_str() {
        "accounts" => list_accounts::run_list_accounts(context),
        "categories" => list_categories::run_list_categories(context),
        "transactions" => list_transactions::run_list_transactions(context),
        "simulations" => list_simulations::run_list_simulations(context),
        "ledgers" => list_ledgers::run_list_ledgers(context),
        "backups" => list_backups::run_list_backups(context),
        "recurring" => list_recurring::run_list_recurring(context),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown list target `{}`",
            other
        ))),
    }
}
