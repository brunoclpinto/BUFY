use crate::CoreError;

pub struct CategoryService;

impl CategoryService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "category service not yet implemented".into(),
        ))
    }
}
