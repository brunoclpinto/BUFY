use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub trait Identifiable {
    fn id(&self) -> Uuid;
}

pub trait NamedEntity {
    fn name(&self) -> &str;
}

pub trait BelongsToCategory {
    fn category_id(&self) -> Option<Uuid>;
}

pub trait Amounted {
    fn amount(&self) -> f64;
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetPeriod {
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Custom(u32),
}

impl BudgetPeriod {
    pub fn days(self) -> Option<u32> {
        match self {
            BudgetPeriod::Daily => Some(1),
            BudgetPeriod::Weekly => Some(7),
            BudgetPeriod::Monthly => Some(30),
            BudgetPeriod::Yearly => Some(365),
            BudgetPeriod::Custom(value) => Some(value.max(1)),
        }
    }
}

impl Default for BudgetPeriod {
    fn default() -> Self {
        BudgetPeriod::Monthly
    }
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

pub use chrono;
pub use serde;
pub use uuid;
