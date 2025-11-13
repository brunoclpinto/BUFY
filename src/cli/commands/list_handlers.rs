use crate::cli::core::{CommandError, CommandResult, ShellContext};

use super::ledger_handlers;
use super::simulation_handlers;

pub fn dispatch(context: &mut ShellContext, key: &str) -> CommandResult {
    match key.to_ascii_lowercase().as_str() {
        "accounts" => context.list_accounts(),
        "categories" => context.list_categories(),
        "transactions" => context.list_transactions(),
        "simulations" => simulation_handlers::list_simulations(context),
        "ledgers" => ledger_handlers::handle_overview(context),
        "backups" => ledger_handlers::handle_list_backups(context, &[]),
        other => Err(CommandError::InvalidArguments(format!(
            "unknown list target `{}`",
            other
        ))),
    }
}
