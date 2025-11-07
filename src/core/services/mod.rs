pub mod account_service;
pub mod category_service;
pub mod summary_service;
pub mod transaction_service;

pub use account_service::AccountService;
pub use category_service::CategoryService;
pub use summary_service::SummaryService;
pub use transaction_service::TransactionService;

use crate::errors::LedgerError;

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error("{0}")]
    Invalid(String),
}
