use crate::CoreError;

pub struct SimulationService;

impl SimulationService {
    pub fn placeholder() -> Result<(), CoreError> {
        Err(CoreError::InvalidOperation(
            "simulation service not yet implemented".into(),
        ))
    }
}
