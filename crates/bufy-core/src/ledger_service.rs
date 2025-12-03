use crate::CoreError;

pub struct LedgerService;

impl LedgerService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "ledger service not yet implemented".into(),
        ))
    }
}
