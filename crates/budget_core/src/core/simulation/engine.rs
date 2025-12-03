use chrono::Utc;
use tracing::warn;

use crate::{
    core::errors::BudgetError,
    ledger::{Ledger, Transaction},
};

use super::types::{
    SimulatedChange, Simulation, SimulationChange, SimulationStatus, SimulationTransactionPatch,
};

pub struct SimulationEngine;

impl SimulationEngine {
    pub fn run(ledger: &Ledger, sim: &Simulation) -> Ledger {
        let mut clone = ledger.clone();
        if let Err(err) = Self::apply_changes(&mut clone.transactions, &sim.changes) {
            warn!(
                "simulation `{}` failed to apply while running preview: {}",
                sim.name, err
            );
        }
        clone
    }

    pub fn diff(ledger: &Ledger, sim: &Simulation) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "Simulation `{}` contains {} change(s)",
            sim.name,
            sim.changes.len()
        ));
        for change in &sim.changes {
            let summary: SimulatedChange = change.into();
            lines.push(format!(
                "- {:?} {} (Δ {:.2})",
                summary.change_type, summary.target_id, summary.delta
            ));
        }
        let simulated = Self::run(ledger, sim);
        let delta = simulated.transactions.len() as isize - ledger.transactions.len() as isize;
        if delta != 0 {
            lines.push(format!("Transactions delta: {}", delta));
        }
        lines
    }

    pub fn apply(ledger: &mut Ledger, simulation: &mut Simulation) -> Result<(), BudgetError> {
        if simulation.status != SimulationStatus::Pending {
            return Err(BudgetError::InvalidInput(format!(
                "simulation `{}` is not pending",
                simulation.name
            )));
        }

        Self::apply_changes(&mut ledger.transactions, &simulation.changes)?;
        ledger.refresh_recurrence_metadata();

        let now = Utc::now();
        simulation.status = SimulationStatus::Applied;
        simulation.applied_at = Some(now);
        simulation.updated_at = now;
        Ok(())
    }

    fn apply_changes(
        transactions: &mut Vec<Transaction>,
        changes: &[SimulationChange],
    ) -> Result<(), BudgetError> {
        for change in changes {
            match change {
                SimulationChange::AddTransaction { transaction } => {
                    transactions.push(transaction.clone());
                }
                SimulationChange::ModifyTransaction(patch) => {
                    let txn = transactions
                        .iter_mut()
                        .find(|t| t.id == patch.transaction_id)
                        .ok_or_else(|| {
                            BudgetError::InvalidReference(format!(
                                "transaction {} not found",
                                patch.transaction_id
                            ))
                        })?;
                    apply_patch(txn, patch);
                }
                SimulationChange::ExcludeTransaction { transaction_id } => {
                    let before = transactions.len();
                    transactions.retain(|t| t.id != *transaction_id);
                    if before == transactions.len() {
                        return Err(BudgetError::InvalidReference(format!(
                            "transaction {} not found",
                            transaction_id
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

fn apply_patch(txn: &mut Transaction, patch: &SimulationTransactionPatch) {
    if let Some(account) = patch.from_account {
        txn.from_account = account;
    }
    if let Some(account) = patch.to_account {
        txn.to_account = account;
    }
    if let Some(category) = patch.category_id {
        txn.category_id = category;
    }
    if let Some(date) = patch.scheduled_date {
        txn.scheduled_date = date;
    }
    if let Some(actual_date) = patch.actual_date {
        txn.actual_date = actual_date;
    }
    if let Some(amount) = patch.budgeted_amount {
        txn.budgeted_amount = amount;
    }
    if let Some(amount) = patch.actual_amount {
        txn.actual_amount = amount;
    }
}
