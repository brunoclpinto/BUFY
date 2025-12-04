use chrono::NaiveDate;
use uuid::Uuid;

use bufy_domain::{
    ledger::{BudgetScope, BudgetSummary, CategoryBudgetStatus, DateWindow},
    simulation::{
        Simulation, SimulationBudgetImpact, SimulationChange, SimulationTransactionPatch,
    },
    transaction::Transaction,
    ForecastReport, Ledger,
};

use bufy_core::{
    BudgetService, Clock, CoreError, ForecastService, SimulationService, SummaryService,
};

/// Provides higher-level helpers that previously lived on [`Ledger`].
pub trait LedgerExt {
    fn budget_window_for(&self, reference: NaiveDate) -> DateWindow;
    fn summarize_period_containing(&self, date: NaiveDate) -> BudgetSummary;
    fn category_budget_statuses_current(&self, clock: &dyn Clock) -> Vec<CategoryBudgetStatus>;
    fn forecast_window_report(
        &self,
        window: DateWindow,
        reference: NaiveDate,
        simulation: Option<&str>,
    ) -> Result<ForecastReport, CoreError>;
    fn summarize_simulation_in_window(
        &self,
        simulation_name: &str,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Result<SimulationBudgetImpact, CoreError>;
    fn summarize_simulation_current(
        &self,
        simulation_name: &str,
        clock: &dyn Clock,
    ) -> Result<SimulationBudgetImpact, CoreError>;
    fn create_simulation(
        &mut self,
        name: impl Into<String>,
        notes: Option<String>,
        clock: &dyn Clock,
    ) -> Result<&Simulation, CoreError>;
    fn add_simulation_transaction(
        &mut self,
        sim_name: &str,
        transaction: Transaction,
    ) -> Result<(), CoreError>;
    fn exclude_transaction_in_simulation(
        &mut self,
        sim_name: &str,
        transaction_id: Uuid,
    ) -> Result<(), CoreError>;
    fn modify_transaction_in_simulation(
        &mut self,
        sim_name: &str,
        patch: SimulationTransactionPatch,
    ) -> Result<(), CoreError>;
    fn apply_simulation(&mut self, sim_name: &str, clock: &dyn Clock) -> Result<(), CoreError>;
    fn discard_simulation(&mut self, sim_name: &str) -> Result<(), CoreError>;
    fn simulation_changes(&self, sim_name: &str) -> Result<&[SimulationChange], CoreError>;
}

impl LedgerExt for Ledger {
    fn budget_window_for(&self, reference: NaiveDate) -> DateWindow {
        self.budget_window_containing(reference)
    }

    fn summarize_period_containing(&self, date: NaiveDate) -> BudgetSummary {
        BudgetService::summarize_period_containing(self, date)
    }

    fn category_budget_statuses_current(&self, clock: &dyn Clock) -> Vec<CategoryBudgetStatus> {
        SummaryService::current_category_budget_statuses(self, clock)
    }

    fn forecast_window_report(
        &self,
        window: DateWindow,
        reference: NaiveDate,
        simulation: Option<&str>,
    ) -> Result<ForecastReport, CoreError> {
        ForecastService::window_report(self, window, reference, simulation)
    }

    fn summarize_simulation_in_window(
        &self,
        simulation_name: &str,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Result<SimulationBudgetImpact, CoreError> {
        SimulationService::summarize_in_window(self, simulation_name, window, scope)
    }

    fn summarize_simulation_current(
        &self,
        simulation_name: &str,
        clock: &dyn Clock,
    ) -> Result<SimulationBudgetImpact, CoreError> {
        let today = clock.today();
        let window = self.budget_window_containing(today);
        let scope = window.scope(today);
        SimulationService::summarize_in_window(self, simulation_name, window, scope)
    }

    fn create_simulation(
        &mut self,
        name: impl Into<String>,
        notes: Option<String>,
        clock: &dyn Clock,
    ) -> Result<&Simulation, CoreError> {
        SimulationService::create(self, name, notes, clock)
    }

    fn add_simulation_transaction(
        &mut self,
        sim_name: &str,
        transaction: Transaction,
    ) -> Result<(), CoreError> {
        SimulationService::add_transaction(self, sim_name, transaction)
    }

    fn exclude_transaction_in_simulation(
        &mut self,
        sim_name: &str,
        transaction_id: Uuid,
    ) -> Result<(), CoreError> {
        SimulationService::exclude_transaction(self, sim_name, transaction_id)
    }

    fn modify_transaction_in_simulation(
        &mut self,
        sim_name: &str,
        patch: SimulationTransactionPatch,
    ) -> Result<(), CoreError> {
        SimulationService::modify_transaction(self, sim_name, patch)
    }

    fn apply_simulation(&mut self, sim_name: &str, clock: &dyn Clock) -> Result<(), CoreError> {
        SimulationService::apply(self, sim_name, clock)
    }

    fn discard_simulation(&mut self, sim_name: &str) -> Result<(), CoreError> {
        SimulationService::discard(self, sim_name)
    }

    fn simulation_changes(&self, sim_name: &str) -> Result<&[SimulationChange], CoreError> {
        SimulationService::changes(self, sim_name)
    }
}
