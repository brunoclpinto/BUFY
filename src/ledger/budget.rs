use super::time_interval::TimeInterval;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Describes an envelope of planned spending for a category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub id: Uuid,
    pub category_id: Uuid,
    pub limit_amount: f64,
    pub recurrence: TimeInterval,
    pub is_active: bool,
}

impl Budget {
    pub fn new(category_id: Uuid, limit_amount: f64, recurrence: TimeInterval) -> Self {
        Self {
            id: Uuid::new_v4(),
            category_id,
            limit_amount,
            recurrence,
            is_active: true,
        }
    }
}
