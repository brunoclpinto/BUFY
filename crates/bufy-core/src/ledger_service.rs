//! Helper functions for high-level ledger orchestration.

use chrono::NaiveDate;

use bufy_domain::{ledger::DateWindow, Ledger, LedgerBudgetPeriod};

/// Provides constructor and mutation helpers for [`Ledger`] instances.
pub struct LedgerService;

impl LedgerService {
    /// Creates a new ledger with the supplied name and budgeting period.
    pub fn create(name: impl Into<String>, period: LedgerBudgetPeriod) -> Ledger {
        Ledger::new(name, period)
    }

    /// Renames a ledger.
    pub fn rename(ledger: &mut Ledger, new_name: impl Into<String>) {
        ledger.name = new_name.into();
        ledger.touch();
    }

    /// Updates the budgeting cadence for the ledger.
    pub fn set_budget_period(ledger: &mut Ledger, period: LedgerBudgetPeriod) {
        ledger.budget_period = period;
        ledger.touch();
    }

    /// Returns the budgeting window that contains `reference`.
    pub fn budget_window_containing(ledger: &Ledger, reference: NaiveDate) -> DateWindow {
        ledger.budget_window_containing(reference)
    }
}
