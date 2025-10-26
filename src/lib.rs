#![doc(test(attr(deny(warnings))))]

//! Budget Core offers foundational ledger, budgeting, and simulation primitives
//! that power higher level budgeting workflows and CLIs.

pub mod cli;
pub mod currency;
pub mod errors;
pub mod ledger;
pub mod simulation;
pub mod utils;

use std::sync::Once;

static INIT_TRACING: Once = Once::new();

/// Initializes global tracing and emits a startup info log.
pub fn init() {
    INIT_TRACING.call_once(|| {
        utils::init_tracing();
        tracing::info!("Budget Core tracing initialized.");
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn init_does_not_panic() {
        super::init();
    }
}
