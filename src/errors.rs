use thiserror::Error;

/// Error type that captures common ledger failures.
#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Invalid reference: {0}")]
    InvalidRef(String),
}
