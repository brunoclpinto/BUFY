use crate::CoreError;

pub struct TransactionService;

impl TransactionService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "transaction service not yet implemented".into(),
        ))
    }
}
