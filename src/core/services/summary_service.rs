use chrono::NaiveDate;

use crate::domain::ledger::{BudgetScope, BudgetSummary, DateWindow, SimulationBudgetImpact};
use crate::ledger::{ForecastReport, Ledger};

use super::{ServiceError, ServiceResult};

pub struct SummaryService;

impl SummaryService {
    pub fn current_totals(ledger: &Ledger) -> BudgetSummary {
        ledger.summarize_current_period()
    }

    pub fn summarize_window(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
    ) -> BudgetSummary {
        ledger.summarize_window_scope(window, scope)
    }

    pub fn summarize_simulation(
        ledger: &Ledger,
        simulation_name: &str,
        window: DateWindow,
        scope: BudgetScope,
    ) -> ServiceResult<SimulationBudgetImpact> {
        ledger
            .summarize_simulation_in_window(simulation_name, window, scope)
            .map_err(ServiceError::from)
    }

    pub fn forecast_window(
        ledger: &Ledger,
        window: DateWindow,
        reference: NaiveDate,
        simulation: Option<&str>,
    ) -> ServiceResult<ForecastReport> {
        ledger
            .forecast_window_report(window, reference, simulation)
            .map_err(ServiceError::from)
    }
}
