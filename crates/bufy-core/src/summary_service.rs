use crate::CoreError;

pub struct SummaryService;

impl SummaryService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "summary service not yet implemented".into(),
        ))
    }
}
