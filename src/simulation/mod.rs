use crate::ledger::Ledger;

/// A lightweight snapshot describing ledger activity, useful for early simulations.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SimulationSummary {
    pub transaction_count: usize,
}

/// Builds a summary view used by future simulation features.
pub fn summarize(ledger: &Ledger) -> SimulationSummary {
    SimulationSummary {
        transaction_count: ledger.transaction_count(),
    }
}
