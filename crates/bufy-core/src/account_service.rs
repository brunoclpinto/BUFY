use crate::CoreError;

pub struct AccountService;

impl AccountService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "account service not yet implemented".into(),
        ))
    }
}
