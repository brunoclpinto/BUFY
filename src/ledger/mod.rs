//! Ledger domain models, persistence-friendly types, and helpers.

pub mod account;
pub mod budget;
pub mod category;
#[allow(clippy::module_inception)]
pub mod ledger;
pub mod recurring;
pub mod time_interval;
pub mod transaction;

pub use account::{Account, AccountKind};
pub use budget::Budget;
pub use category::{Category, CategoryKind};
pub use ledger::{
    AccountBudget, BudgetPeriod, BudgetScope, BudgetStatus, BudgetSummary, BudgetTotals,
    DateWindow, ForecastReport, Ledger, Simulation, SimulationBudgetImpact, SimulationChange,
    SimulationStatus, SimulationTransactionPatch,
};
pub use recurring::{
    ForecastResult, ForecastTotals, ForecastTransaction, RecurrenceSnapshot, ScheduledStatus,
};
pub use time_interval::{TimeInterval, TimeUnit};
pub use transaction::{
    Recurrence, RecurrenceEnd, RecurrenceMode, RecurrenceStatus, Transaction, TransactionStatus,
};
