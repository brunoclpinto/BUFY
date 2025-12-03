use std::result::Result as StdResult;

use bufy_config::ConfigError as CliConfigError;
use bufy_core::CoreError as ServiceCoreError;
use thiserror::Error;

/// Unified error type for core/domain/storage layers.
#[derive(Error, Debug)]
pub enum BudgetError {
    #[error("Ledger not loaded")]
    LedgerNotLoaded,
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Category not found: {0}")]
    CategoryNotFound(String),
    #[error("Transaction failed: {0}")]
    TransactionError(String),
    #[error("Persistence error: {0}")]
    StorageError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Invalid reference: {0}")]
    InvalidReference(String),
}

pub type Result<T> = StdResult<T, BudgetError>;

/// User-facing CLI error wrapper.
#[derive(Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    Core(#[from] BudgetError),
    #[error("Invalid input: {0}")]
    Input(String),
    #[error("Command failed: {0}")]
    Command(String),
}

impl From<std::io::Error> for BudgetError {
    fn from(err: std::io::Error) -> Self {
        BudgetError::StorageError(err.to_string())
    }
}

impl From<serde_json::Error> for BudgetError {
    fn from(err: serde_json::Error) -> Self {
        BudgetError::StorageError(err.to_string())
    }
}

impl From<bufy_domain::ledger::DateWindowError> for BudgetError {
    fn from(err: bufy_domain::ledger::DateWindowError) -> Self {
        BudgetError::InvalidInput(err.to_string())
    }
}

impl From<ServiceCoreError> for BudgetError {
    fn from(err: ServiceCoreError) -> Self {
        match err {
            ServiceCoreError::LedgerNotLoaded => BudgetError::LedgerNotLoaded,
            ServiceCoreError::LedgerNotFound(message)
            | ServiceCoreError::Storage(message)
            | ServiceCoreError::Serde(message) => BudgetError::StorageError(message),
            ServiceCoreError::AccountNotFound(message) => BudgetError::AccountNotFound(message),
            ServiceCoreError::CategoryNotFound(message) => BudgetError::CategoryNotFound(message),
            ServiceCoreError::TransactionNotFound(id) => {
                BudgetError::TransactionError(format!("transaction {} not found", id))
            }
            ServiceCoreError::SimulationNotFound(message)
            | ServiceCoreError::InvalidOperation(message)
            | ServiceCoreError::Validation(message) => BudgetError::InvalidInput(message),
            ServiceCoreError::Io(err) => BudgetError::StorageError(err.to_string()),
        }
    }
}

impl From<CliConfigError> for BudgetError {
    fn from(err: CliConfigError) -> Self {
        match err {
            CliConfigError::Io(io) => BudgetError::StorageError(io.to_string()),
            CliConfigError::Serde(message) => BudgetError::ConfigError(message),
        }
    }
}

impl From<CliConfigError> for CliError {
    fn from(err: CliConfigError) -> Self {
        CliError::from(BudgetError::from(err))
    }
}
