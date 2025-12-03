use std::io;

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Ledger not loaded")]
    LedgerNotLoaded,
    #[error("Ledger not found: {0}")]
    LedgerNotFound(String),
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Category not found: {0}")]
    CategoryNotFound(String),
    #[error("Transaction not found: {0}")]
    TransactionNotFound(Uuid),
    #[error("Simulation not found: {0}")]
    SimulationNotFound(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serde(String),
}
