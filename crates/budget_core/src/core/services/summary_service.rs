//! Aggregation helpers for budgeting summaries and forecasts.

use chrono::NaiveDate;

use crate::core::services::{
    BudgetService, CategoryBudgetAssignment, CategoryBudgetStatus, CategoryBudgetSummary,
    CategoryBudgetSummaryKind, ServiceError, ServiceResult,
};
use bufy_domain::ledger::{BudgetScope, BudgetSummary, DateWindow};
use crate::ledger::{ForecastReport, Ledger, SimulationBudgetImpact};

/// Aggregates ledger data for summary and forecasting scenarios.
///
/// See also: [`bufy_domain::ledger::BudgetSummary`] for the returned data model.
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
        ledger.category_budget_statuses(window, scope)
    }

    /// Returns category budget usage for the ledger's current budgeting period.
    pub fn current_category_budget_statuses(ledger: &Ledger) -> Vec<CategoryBudgetStatus> {
        ledger.category_budget_statuses_current()
    }

    /// Lists every category with an explicit budget assignment.
    pub fn categories_with_budgets(ledger: &Ledger) -> Vec<CategoryBudgetAssignment> {
        ledger.categories_with_budgets()
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
    ) -> ServiceResult<SimulationBudgetImpact> {
        ledger
            .summarize_simulation_in_window(simulation_name, window, scope)
            .map_err(ServiceError::from)
    }

    /// Produces a forecast report for the given window and optional simulation.
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

#[cfg(test)]
mod tests {
    use crate::core::services::{CategoryBudgetSummaryKind, SummaryService};
    use bufy_domain::{
        account::{Account, AccountKind},
        category::{Category, CategoryKind},
        ledger::{BudgetScope, DateWindow},
        transaction::{Recurrence, RecurrenceMode, TransactionStatus},
        BudgetPeriod as CategoryBudgetPeriod,
    };
    use crate::ledger::time_interval::{TimeInterval, TimeUnit};
    use crate::ledger::{Ledger, Transaction};
    use chrono::{Duration, NaiveDate, Utc};
    use uuid::Uuid;

    fn ledger_with_transaction() -> Ledger {
        let mut ledger = Ledger::new("Summary", crate::ledger::BudgetPeriod::monthly());
        let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
        let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut txn = Transaction::new(checking, savings, None, date, 125.0);
        txn.status = TransactionStatus::Planned;
        let recurrence = Recurrence::new(
            date,
            TimeInterval {
                every: 1,
                unit: TimeUnit::Month,
            },
            RecurrenceMode::FixedSchedule,
        );
        txn.set_recurrence(Some(recurrence));
        ledger.add_transaction(txn);
        ledger
    }

    fn ledger_with_category_budget(reference: NaiveDate) -> (Ledger, Uuid) {
        let mut ledger = Ledger::new("CategorySummary", crate::ledger::BudgetPeriod::monthly());
        let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
        let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
        let mut dining = Category::new("Dining", CategoryKind::Expense);
        dining.set_budget(250.0, CategoryBudgetPeriod::Monthly, None);
        let category_id = dining.id;
        ledger.add_category(dining);
        let mut txn = Transaction::new(checking, savings, Some(category_id), reference, 75.0);
        txn.actual_amount = Some(70.0);
        txn.actual_date = Some(reference);
        ledger.add_transaction(txn);
        (ledger, category_id)
    }

    #[test]
    fn forecast_window_reports_expected_transactions() {
        let ledger = ledger_with_transaction();
        let window = DateWindow::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
        .expect("valid window");
        let report = SummaryService::forecast_window(&ledger, window.clone(), window.start, None)
            .expect("forecast succeeds");
        assert_eq!(report.forecast.window, window);
    }

    #[test]
    fn summarize_simulation_errors_for_unknown_name() {
        let ledger = ledger_with_transaction();
        let window = DateWindow::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
        .expect("valid window");
        let err =
            SummaryService::summarize_simulation(&ledger, "missing", window, BudgetScope::Custom)
                .expect_err("missing simulation should fail");
        let message = format!("{err}");
        assert!(message.contains("missing"), "unexpected error: {message}");
    }

    #[test]
    fn category_budget_statuses_surface_budget_data() {
        let reference = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let (ledger, category_id) = ledger_with_category_budget(reference);
        let window = ledger.budget_window_for(reference);
        let scope = window.scope(reference);
        let statuses = SummaryService::category_budget_statuses(&ledger, window, scope);
        let status = statuses
            .into_iter()
            .find(|entry| entry.category_id == category_id)
            .expect("category status present");
        assert_eq!(status.totals.budgeted, 75.0);
        assert_eq!(status.totals.real, 70.0);
        assert_eq!(status.budget.as_ref().map(|b| b.amount), Some(250.0));
    }

    #[test]
    fn current_category_statuses_include_budgeted_categories() {
        let today = Utc::now().date_naive();
        let (ledger, _) = ledger_with_category_budget(today);
        let statuses = SummaryService::current_category_budget_statuses(&ledger);
        assert!(
            !statuses.is_empty(),
            "expected at least one budgeted category in the current window"
        );
    }

    #[test]
    fn category_budget_summaries_expose_utilization() {
        let reference = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let (ledger, _) = ledger_with_category_budget(reference);
        let window = ledger.budget_window_for(reference);
        let scope = window.scope(reference);
        let summaries = SummaryService::category_budget_summaries(&ledger, window, scope);
        assert_eq!(summaries.len(), 1);
        let entry = &summaries[0];
        assert_eq!(entry.kind, CategoryBudgetSummaryKind::Actual);
        assert!((entry.spent_amount - 70.0).abs() < f64::EPSILON);
        assert!(entry.utilization_percent.unwrap() > 20.0);
    }

    #[test]
    fn forecast_includes_category_budget_summaries() {
        let reference = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let (ledger, _) = ledger_with_category_budget(reference);
        let window = DateWindow::new(reference, reference + Duration::days(30)).unwrap();
        let report = SummaryService::forecast_window(&ledger, window, reference, None).unwrap();
        assert!(
            !report.category_budgets.is_empty(),
            "expected projected category budgets"
        );
        assert_eq!(
            report.category_budgets[0].kind,
            CategoryBudgetSummaryKind::Projected
        );
    }

    #[test]
    fn simulation_impact_includes_category_budgets() {
        let reference = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let (mut ledger, category_id) = ledger_with_category_budget(reference);
        let sim_name = "Plan";
        ledger.create_simulation(sim_name, None).unwrap();
        let from = ledger.accounts[0].id;
        let to = ledger.accounts[1].id;
        let mut extra = Transaction::new(from, to, Some(category_id), reference, 30.0);
        extra.actual_amount = Some(25.0);
        extra.actual_date = Some(reference);
        ledger
            .add_simulation_transaction(sim_name, extra)
            .expect("simulation mutation");
        let window = ledger.budget_window_for(reference);
        let scope = window.scope(reference);
        let impact = ledger
            .summarize_simulation_in_window(sim_name, window, scope)
            .expect("simulate");
        assert!(
            !impact.category_budgets_base.is_empty()
                && !impact.category_budgets_simulated.is_empty()
        );
        assert_eq!(
            impact.category_budgets_simulated[0].kind,
            CategoryBudgetSummaryKind::Simulated
        );
    }
}
