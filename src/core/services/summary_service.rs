use chrono::NaiveDate;

use crate::domain::ledger::{BudgetScope, BudgetSummary, DateWindow};
use crate::ledger::{ForecastReport, Ledger, SimulationBudgetImpact};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        account::{Account, AccountKind},
        ledger::{BudgetScope, DateWindow},
        transaction::{Recurrence, RecurrenceMode, TransactionStatus},
    };
    use crate::ledger::time_interval::{TimeInterval, TimeUnit};
    use crate::ledger::{Ledger, Transaction};
    use chrono::NaiveDate;

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
}
