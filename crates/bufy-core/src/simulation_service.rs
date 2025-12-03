//! Simulation orchestration helpers built on top of the domain ledger.

use chrono::Utc;
use uuid::Uuid;

use bufy_domain::{
    ledger::{
        BudgetScope, BudgetTotalsDelta, CategoryBudgetSummaryKind, DateWindow,
    },
    simulation::{
        Simulation, SimulationBudgetImpact, SimulationChange, SimulationStatus,
        SimulationTransactionPatch,
    },
    transaction::Transaction,
    Ledger,
};

use crate::budget_service::BudgetService;
use crate::CoreError;

pub struct SimulationService;

impl SimulationService {
    /// Creates a new simulation within the ledger.
    pub fn create(
        ledger: &mut Ledger,
        name: impl Into<String>,
        notes: Option<String>,
    ) -> Result<&Simulation, CoreError> {
        let name = name.into();
        if ledger
            .simulations()
            .iter()
            .any(|sim| sim.name.eq_ignore_ascii_case(&name))
        {
            return Err(CoreError::Validation(format!(
                "simulation `{}` already exists",
                name
            )));
        }
        let now = Utc::now();
        ledger.simulations.push(Simulation {
            id: Uuid::new_v4(),
            name,
            notes,
            status: SimulationStatus::Pending,
            created_at: now,
            updated_at: now,
            applied_at: None,
            changes: Vec::new(),
        });
        ledger.touch();
        Ok(ledger
            .simulations()
            .last()
            .expect("simulation just inserted"))
    }

    /// Adds a transaction change to a simulation.
    pub fn add_transaction(
        ledger: &mut Ledger,
        sim_name: &str,
        transaction: Transaction,
    ) -> Result<(), CoreError> {
        if ledger.add_simulation_transaction_raw(sim_name, transaction) {
            Ok(())
        } else {
            Err(CoreError::SimulationNotFound(sim_name.into()))
        }
    }

    /// Excludes a transaction from a simulation overlay.
    pub fn exclude_transaction(
        ledger: &mut Ledger,
        sim_name: &str,
        transaction_id: Uuid,
    ) -> Result<(), CoreError> {
        if !ledger
            .transactions
            .iter()
            .any(|txn| txn.id == transaction_id)
        {
            return Err(CoreError::TransactionNotFound(transaction_id));
        }
        if ledger.exclude_transaction_in_simulation_raw(sim_name, transaction_id) {
            Ok(())
        } else {
            Err(CoreError::SimulationNotFound(sim_name.into()))
        }
    }

    /// Applies a partial modification to an existing transaction.
    pub fn modify_transaction(
        ledger: &mut Ledger,
        sim_name: &str,
        patch: SimulationTransactionPatch,
    ) -> Result<(), CoreError> {
        if !ledger
            .transactions
            .iter()
            .any(|txn| txn.id == patch.transaction_id)
        {
            return Err(CoreError::TransactionNotFound(patch.transaction_id));
        }
        if ledger.modify_transaction_in_simulation_raw(sim_name, patch) {
            Ok(())
        } else {
            Err(CoreError::SimulationNotFound(sim_name.into()))
        }
    }

    /// Removes an entire simulation by name.
    pub fn discard(ledger: &mut Ledger, sim_name: &str) -> Result<(), CoreError> {
        if ledger.discard_simulation_raw(sim_name) {
            Ok(())
        } else {
            Err(CoreError::SimulationNotFound(sim_name.into()))
        }
    }

    /// Applies a simulation, mutating the ledger transactions.
    pub fn apply(ledger: &mut Ledger, sim_name: &str) -> Result<(), CoreError> {
        let index = ledger
            .simulations()
            .iter()
            .position(|sim| sim.name.eq_ignore_ascii_case(sim_name))
            .ok_or_else(|| CoreError::SimulationNotFound(sim_name.into()))?;
        let mut simulation = ledger.simulations.remove(index);
        SimulationEngine::apply(ledger, &mut simulation)?;
        ledger.simulations.insert(index, simulation);
        ledger.touch();
        Ok(())
    }

    /// Returns the list of changes recorded in the simulation.
    pub fn changes<'a>(
        ledger: &'a Ledger,
        sim_name: &str,
    ) -> Result<&'a [SimulationChange], CoreError> {
        ledger
            .simulation_changes_raw(sim_name)
            .ok_or_else(|| CoreError::SimulationNotFound(sim_name.into()))
    }

    /// Runs a simulation against the ledger, returning an overlay ledger.
    pub fn run(ledger: &Ledger, sim_name: &str) -> Result<Ledger, CoreError> {
        let simulation = ledger
            .simulation(sim_name)
            .ok_or_else(|| CoreError::SimulationNotFound(sim_name.into()))?;
        Ok(SimulationEngine::run(ledger, simulation))
    }

    /// Summarizes the effect of a simulation in a window.
    pub fn summarize_in_window(
        ledger: &Ledger,
        simulation_name: &str,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Result<SimulationBudgetImpact, CoreError> {
        let simulation = ledger
            .simulation(simulation_name)
            .ok_or_else(|| CoreError::SimulationNotFound(simulation_name.into()))?;
        if simulation.status == SimulationStatus::Discarded {
            return Err(CoreError::InvalidOperation(format!(
                "simulation `{}` is discarded",
                simulation_name
            )));
        }
        let simulated_ledger = SimulationEngine::run(ledger, simulation);
        let base = BudgetService::summarize_window_scope(ledger, window, scope);
        let simulated = BudgetService::summarize_window_scope(&simulated_ledger, window, scope);
        let delta = BudgetTotalsDelta {
            budgeted: simulated.totals.budgeted - base.totals.budgeted,
            real: simulated.totals.real - base.totals.real,
            remaining: simulated.totals.remaining - base.totals.remaining,
            variance: simulated.totals.variance - base.totals.variance,
        };
        let base_category_budgets = BudgetService::category_budget_summaries(
            ledger,
            window,
            scope,
            CategoryBudgetSummaryKind::Actual,
        );
        let simulated_category_budgets = BudgetService::category_budget_summaries(
            &simulated_ledger,
            window,
            scope,
            CategoryBudgetSummaryKind::Simulated,
        );
        Ok(SimulationBudgetImpact {
            simulation_name: simulation.name.clone(),
            base,
            simulated,
            delta,
            category_budgets_base: base_category_budgets,
            category_budgets_simulated: simulated_category_budgets,
        })
    }
}

struct SimulationEngine;

impl SimulationEngine {
    fn run(ledger: &Ledger, sim: &Simulation) -> Ledger {
        let mut clone = ledger.clone();
        if Self::apply_changes(&mut clone.transactions, &sim.changes).is_err() {
            // Ignore failures when building preview copies; validation happens when applying.
        }
        clone
    }

    fn apply(ledger: &mut Ledger, simulation: &mut Simulation) -> Result<(), CoreError> {
        if simulation.status != SimulationStatus::Pending {
            return Err(CoreError::InvalidOperation(format!(
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
    ) -> Result<(), CoreError> {
        for change in changes {
            match change {
                SimulationChange::AddTransaction { transaction } => {
                    transactions.push(transaction.clone());
                }
                SimulationChange::ModifyTransaction(patch) => {
                    let txn = transactions
                        .iter_mut()
                        .find(|t| t.id == patch.transaction_id)
                        .ok_or_else(|| CoreError::TransactionNotFound(patch.transaction_id))?;
                    apply_patch(txn, patch);
                }
                SimulationChange::ExcludeTransaction { transaction_id } => {
                    let before = transactions.len();
                    transactions.retain(|t| t.id != *transaction_id);
                    if before == transactions.len() {
                        return Err(CoreError::TransactionNotFound(*transaction_id));
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
