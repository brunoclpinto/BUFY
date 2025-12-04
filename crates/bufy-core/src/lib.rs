//! bufy-core
//!
//! Business logic and services for BUFΥ.
//! Depends on bufy-domain. No CLI, no terminal I/O, no direct storage interactions.

pub mod account_service;
pub mod budget_service;
pub mod category_service;
pub mod error;
pub mod forecast_service;
pub mod format;
pub mod ledger_service;
pub mod public_api;
pub mod recurrence_service;
pub mod simulation_service;
pub mod storage;
pub mod summary_service;
pub mod time;
pub mod transaction_service;

pub use account_service::*;
pub use budget_service::*;
pub use category_service::*;
pub use error::CoreError;
pub use forecast_service::*;
pub use format::{CurrencyFormatter, DateFormatter};
pub use ledger_service::*;
pub use public_api::*;
pub use recurrence_service::*;
pub use simulation_service::*;
pub use storage::*;
pub use summary_service::*;
pub use time::Clock;
pub use transaction_service::*;
