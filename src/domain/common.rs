pub use chrono::NaiveDate;
pub use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use uuid::Uuid;

/// Identifies entities that expose a stable unique identifier.
pub trait Identifiable {
    fn id(&self) -> Uuid;
}

/// Provides access to a human-friendly entity name.
pub trait NamedEntity {
    fn name(&self) -> &str;
}

/// Supplies a presentation-ready label for UI or logs.
pub trait Displayable {
    fn display_label(&self) -> String;
}

// Placeholder error type hook for future validation rules.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

// Re-export chrono/serde so downstream modules can import from a single place.
pub use chrono;
pub use serde;
pub use uuid;
