pub mod account_service;
pub mod budget_service;
pub mod category_service;
pub mod summary_service;
pub mod transaction_service;

pub use account_service::AccountService;
pub use budget_service::BudgetService;
pub use bufy_domain::{
    CategoryBudgetAssignment, CategoryBudgetStatus, CategoryBudgetSummary,
    CategoryBudgetSummaryKind,
};
pub use category_service::CategoryService;
pub use summary_service::SummaryService;
pub use transaction_service::TransactionService;

use crate::core::errors::BudgetError;

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(transparent)]
    Core(#[from] BudgetError),
    #[error("{0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests;
