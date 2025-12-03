use crate::CoreError;

pub struct BudgetService;

impl BudgetService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "budget service not yet implemented".into(),
        ))
    }
}
