//! bufy-core
//!
//! Business logic and services for BUFΥ.
//! Depends on bufy-domain. No CLI, no terminal I/O, no direct storage interactions.

pub mod error;
pub mod ledger_service;
pub mod account_service;
pub mod category_service;
pub mod transaction_service;
pub mod simulation_service;
pub mod summary_service;
pub mod budget_service;
pub mod forecast_service;

pub use error::CoreError;
pub use ledger_service::*;
pub use account_service::*;
pub use category_service::*;
pub use transaction_service::*;
pub use simulation_service::*;
pub use summary_service::*;
pub use budget_service::*;
pub use forecast_service::*;
