//! Forecasting helpers that combine recurring schedules with ledger data.

use chrono::NaiveDate;

use bufy_domain::{
    ledger::{CategoryBudgetSummaryKind, DateWindow},
    recurring::forecast_for_window,
    ForecastReport, Ledger,
};

use crate::{budget_service::BudgetService, simulation_service::SimulationService, CoreError};

pub struct ForecastService;

impl ForecastService {
    /// Produces a forecast report for the given window and optional simulation overlay.
    pub fn window_report(
        ledger: &Ledger,
        window: DateWindow,
        reference: NaiveDate,
        simulation: Option<&str>,
    ) -> Result<ForecastReport, CoreError> {
        let scope = window.scope(reference);
        let base_transactions = if let Some(name) = simulation {
            SimulationService::run(ledger, name)?.transactions
        } else {
            ledger.transactions.clone()
        };
        let forecast = forecast_for_window(window, reference, &base_transactions);
        let mut overlay = base_transactions.clone();
        overlay.extend(
            forecast
                .transactions
                .iter()
                .map(|item| item.transaction.clone()),
        );
        let summary =
            BudgetService::summarize_window_with_transactions(ledger, window, scope, &overlay);
        let category_budgets = BudgetService::category_budget_summaries_with_transactions(
            ledger,
            window,
            scope,
            Some(&overlay),
            CategoryBudgetSummaryKind::Projected,
        );
        Ok(ForecastReport {
            scope,
            forecast,
            summary,
            category_budgets,
        })
    }
}
