//! Placeholder module for wizard-style form flows.
//!
//! Phase 14 will introduce concrete form runners. The definitions here
//! capture the expected interface so the rest of the CLI can depend on a
//! stable contract.

/// Represents a multi-step data entry workflow that collects domain values.
pub trait FormFlow {
    type Output;
    type Error;

    /// Executes the form, walking through each step until completion or error.
    ///
    /// Implementations should surface validation feedback consistently and may
    /// provide defaults where appropriate. Returning `Ok(Output)` indicates the
    /// form completed successfully; returning `Err(Error)` signals cancellation
    /// or validation failure.
    fn run(&mut self) -> Result<Self::Output, Self::Error>;
}
