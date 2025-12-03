//! Ledger domain models, persistence-friendly types, and helpers.

pub mod account;
pub mod budget;
pub mod category;
#[allow(clippy::module_inception)]
pub mod ledger;
pub mod recurring;
pub mod time_interval;
pub mod transaction;

pub use bufy_domain::{
    CategoryBudgetAssignment, CategoryBudgetStatus, CategoryBudgetSummary,
    CategoryBudgetSummaryKind,
};
pub use bufy_domain::simulation::{
    Simulation, SimulationBudgetImpact, SimulationChange, SimulationStatus,
    SimulationTransactionPatch,
};
pub use account::{Account, AccountKind};
pub use budget::Budget;
pub use category::{Category, CategoryBudgetDefinition, CategoryKind};
pub use ledger::{
    AccountBudget, BudgetPeriod, BudgetScope, BudgetStatus, BudgetSummary, BudgetTotals,
    BudgetTotalsDelta, DateWindow, ForecastReport, Ledger,
};
pub use recurring::{
    ForecastResult, ForecastTotals, ForecastTransaction, RecurrenceSnapshot, ScheduledStatus,
};
pub use time_interval::{TimeInterval, TimeUnit};
pub use transaction::{
    Recurrence, RecurrenceEnd, RecurrenceMode, RecurrenceStatus, Transaction, TransactionStatus,
};
