use crate::ledger::{ForecastReport, Ledger};

use super::{ServiceError, ServiceResult};

pub struct SummaryService;

impl SummaryService {
    pub fn current_totals(ledger: &Ledger) -> ServiceResult<()> {
        let _ = ledger;
        Err(ServiceError::Invalid(
            "SummaryService::current_totals not yet implemented".into(),
        ))
    }

    pub fn budget_vs_real(ledger: &Ledger) -> ServiceResult<()> {
        let _ = ledger;
        Err(ServiceError::Invalid(
            "SummaryService::budget_vs_real not yet implemented".into(),
        ))
    }

    pub fn period_summary(ledger: &Ledger) -> ServiceResult<ForecastReport> {
        let _ = ledger;
        Err(ServiceError::Invalid(
            "SummaryService::period_summary not yet implemented".into(),
        ))
    }
}
