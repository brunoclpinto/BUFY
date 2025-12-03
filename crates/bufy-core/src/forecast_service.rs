use crate::CoreError;

pub struct ForecastService;

impl ForecastService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "forecast service not yet implemented".into(),
        ))
    }
}
