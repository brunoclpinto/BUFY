# Budget Core — Design Overview

Budget Core is the domain foundation for a multi-surface budgeting experience. The project starts as a reusable Rust library with a thin CLI harness and will grow to power rich simulations, data ingestion, and reporting workflows. This document introduces the core modules planned for Phase 0 and the guiding principles for future development.

## Architectural Principles

- **Deterministic & auditable**: Ledger operations should be reproducible, validating all inputs and preserving authority for each mutation.
- **Composable domain primitives**: Accounts, categories, budgets, and transactions are isolated modules that can be combined into higher-level workflows.
- **Extensible simulations**: Simulation features consume ledger data through stable APIs, enabling experimentation without mutating authoritative state.
- **Observability-first**: Tracing is available from the earliest boot sequence so that future services inherit consistent telemetry.

## Module Breakdown

### `ledger`

The ledger module owns the domain entities required for bookkeeping:

- `Account` represents asset or liability containers and tracks balances in integer cents.
- `Category` labels transactions for budgeting and reporting.
- `Budget` stores spending guardrails per category and period.
- `Transaction` captures discrete financial movements against accounts. In later phases, validation and balance adjustments will live here.
- `Ledger` aggregates the above entities, offering storing and lookup helpers that surface structured `LedgerError` values.

### `simulation`

The simulation module currently provides lightweight summaries (`SimulationSummary`) of ledger activity. Future iterations will deliver forward-looking projections, Monte Carlo models, and scenario testing. The intent is to keep simulation read-only, consuming ledger data via well-defined APIs.

### `utils`

Utility helpers house cross-cutting concerns. Phase 0 ships the tracing bootstrapper (`init_tracing`) that configures an env-filtered subscriber with the crate defaulting to `info` level. Additional helpers (configuration loading, file IO, etc.) will land here in subsequent phases.

### `errors`

`LedgerError` consolidates domain error reporting using `thiserror` for ergonomic conversions. IO and serialization errors are captured directly, and higher-level operations will extend the enum with contextual variants as functionality expands.

## Next Steps

1. Expand ledger operations with validated mutations that maintain account balances.
2. Introduce persistence adapters (file-based, SQLite) with serde-powered serialization.
3. Layer richer simulations and forecasting utilities, leveraging cached snapshots.
4. Wire additional tooling (benchmarks, fuzzing) as the codebase
   deepens.
