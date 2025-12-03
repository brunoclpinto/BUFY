use crate::cli::core::ShellContext;
use bufy_domain::{simulation::SimulationStatus, transaction::TransactionStatus};

pub(crate) struct MenuContextState {
    pub has_loaded_ledger: bool,
    pub has_named_ledger: bool,
    pub has_accounts: bool,
    pub has_categories: bool,
    pub has_transactions: bool,
    pub has_planned_transactions: bool,
    pub has_simulations: bool,
    pub has_pending_simulations: bool,
    pub has_active_simulation: bool,
}

impl MenuContextState {
    pub fn capture(context: &ShellContext) -> Self {
        let (has_named_ledger, handle) = {
            let manager = context.manager();
            (manager.current_name().is_some(), manager.current_handle())
        };

        let has_loaded_ledger = handle.is_some();
        let (
            has_accounts,
            has_categories,
            has_transactions,
            has_planned_transactions,
            has_simulations,
            has_pending_simulations,
        ) = if let Some(handle) = handle {
            match handle.read() {
                Ok(ledger) => (
                    !ledger.accounts.is_empty(),
                    !ledger.categories.is_empty(),
                    !ledger.transactions.is_empty(),
                    ledger
                        .transactions
                        .iter()
                        .any(|txn| txn.status == TransactionStatus::Planned),
                    !ledger.simulations.is_empty(),
                    ledger
                        .simulations
                        .iter()
                        .any(|sim| sim.status == SimulationStatus::Pending),
                ),
                Err(_) => (false, false, false, false, false, false),
            }
        } else {
            (false, false, false, false, false, false)
        };

        Self {
            has_loaded_ledger,
            has_named_ledger,
            has_accounts,
            has_categories,
            has_transactions,
            has_planned_transactions,
            has_simulations,
            has_pending_simulations,
            has_active_simulation: context.is_simulation_active(),
        }
    }
}
