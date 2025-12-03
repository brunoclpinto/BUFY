//! Aggregation helpers for budgeting summaries and forecasts.

use chrono::{NaiveDate, Utc};

use bufy_domain::{
    ledger::{
        BudgetScope, BudgetSummary, CategoryBudgetAssignment, CategoryBudgetStatus,
        CategoryBudgetSummary, CategoryBudgetSummaryKind, DateWindow,
    },
    simulation::SimulationBudgetImpact,
    ForecastReport, Ledger,
};

use crate::{
    budget_service::BudgetService, forecast_service::ForecastService,
    simulation_service::SimulationService, CoreError,
};

/// Aggregates ledger data for summary and forecasting scenarios.
pub struct SummaryService;

impl SummaryService {
    /// Summarizes the ledger's current budget window.
    pub fn current_totals(ledger: &Ledger) -> BudgetSummary {
        BudgetService::summarize_current_period(ledger)
    }

    /// Summarizes the supplied window and scope against the ledger.
    pub fn summarize_window(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
    ) -> BudgetSummary {
        BudgetService::summarize_window_scope(ledger, window, scope)
    }

    /// Returns category budget usage for the supplied window.
    pub fn category_budget_statuses(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Vec<CategoryBudgetStatus> {
        BudgetService::category_budget_statuses(ledger, window, scope)
    }

    /// Returns category budget usage for the ledger's current budgeting period.
    pub fn current_category_budget_statuses(ledger: &Ledger) -> Vec<CategoryBudgetStatus> {
        let today = Utc::now().date_naive();
        let window = ledger.budget_window_containing(today);
        let scope = window.scope(today);
        BudgetService::category_budget_statuses(ledger, window, scope)
    }

    /// Lists every category with an explicit budget assignment.
    pub fn categories_with_budgets(ledger: &Ledger) -> Vec<CategoryBudgetAssignment> {
        BudgetService::categories_with_budgets(ledger)
    }

    /// Provides detailed category budget summaries for the supplied window.
    pub fn category_budget_summaries(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Vec<CategoryBudgetSummary> {
        BudgetService::category_budget_summaries(
            ledger,
            window,
            scope,
            CategoryBudgetSummaryKind::Actual,
        )
    }

    /// Summarizes the impact of a simulation in a specific window and scope.
    pub fn summarize_simulation(
        ledger: &Ledger,
        simulation_name: &str,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Result<SimulationBudgetImpact, CoreError> {
        SimulationService::summarize_in_window(ledger, simulation_name, window, scope)
    }

    /// Produces a forecast report for the given window and optional simulation.
    pub fn forecast_window(
        ledger: &Ledger,
        window: DateWindow,
        reference: NaiveDate,
        simulation: Option<&str>,
    ) -> Result<ForecastReport, CoreError> {
        ForecastService::window_report(ledger, window, reference, simulation)
    }
}
