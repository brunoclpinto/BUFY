//! Ledger domain models, persistence-friendly types, and helpers.

pub mod account;
pub mod budget;
pub mod category;
pub mod ext;
pub mod recurring;
pub mod time_interval;
pub mod transaction;

pub use account::{Account, AccountKind};
pub use budget::Budget;
pub use bufy_domain::{
    ledger::{
        AccountBudget, BudgetScope, BudgetStatus, BudgetSummary, BudgetTotals, BudgetTotalsDelta,
        CategoryBudget, CategoryBudgetAssignment, CategoryBudgetStatus, CategoryBudgetSummary,
        CategoryBudgetSummaryKind, DateWindow,
    },
    ledger_data::{
        ConversionContext, CurrencyConversionError, ForecastReport, Ledger, LedgerBudgetPeriod,
    },
    simulation::{
        Simulation, SimulationBudgetImpact, SimulationChange, SimulationStatus,
        SimulationTransactionPatch,
    },
};
pub use category::{Category, CategoryBudgetDefinition, CategoryKind};
pub use ext::LedgerExt;
pub use recurring::{
    ForecastResult, ForecastTotals, ForecastTransaction, RecurrenceSnapshot, ScheduledStatus,
};
pub use time_interval::{TimeInterval, TimeUnit};
pub use transaction::{
    Recurrence, RecurrenceEnd, RecurrenceMode, RecurrenceStatus, Transaction, TransactionStatus,
};
pub use LedgerBudgetPeriod as BudgetPeriod;
