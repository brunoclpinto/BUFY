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
- `TimeInterval` and `TimeUnit` express common recurrence windows that power budgets and transactions alike.
- `Transaction` captures discrete financial movements against accounts and encodes recurrence policies.
- `Ledger` aggregates the above entities, offering storing and lookup helpers that surface structured `LedgerError` values. The structure is serde-friendly for JSON imports/exports while maintaining timestamps and schema versions for migrations. Phase 4 layers budget summaries on the ledger so it can calculate period windows, per-category/account aggregations, variances, and health classifications without involving the CLI.

### `simulation`

The simulation module currently provides lightweight summaries (`SimulationSummary`) of ledger activity. Future iterations will deliver forward-looking projections, Monte Carlo models, and scenario testing. The intent is to keep simulation read-only, consuming ledger data via well-defined APIs.

### `cli`

Phase 3 introduces an interactive `rustyline`-powered shell that wraps the ledger APIs with contextual menus and command dispatch. Users can create or load ledgers, add accounts/categories/transactions, list data, and save progress without leaving the prompt. Time-based fields use a `TimeInterval` editor that supports arbitrary “repeat every N <unit>” entries for budgets and recurrences. Script mode (enabled via the `BUDGET_CORE_CLI_SCRIPT` env var) keeps pipelines testable for CI and automated workflows. The `summary` command delegates directly to the ledger’s budgeting APIs so the shell remains a thin presentation layer.

### Simulations (Phase 5)

Simulations are persisted, name-addressable overlays that store only the delta relative to the authoritative ledger. Each simulation carries metadata (name, notes, timestamps, status) and a list of changes:

- `AddTransaction` — staged hypothetical entries.
- `ModifyTransaction` — partial patches against existing transactions.
- `ExcludeTransaction` — temporarily ignore a real transaction.

Ledger JSON now includes a `simulations` array so scenarios survive reloads and version bumps. The ledger exposes APIs to create, list, summarize, apply, and discard simulations, and budget summaries can optionally include a simulation overlay to show base/simulated totals plus deltas. The CLI surfaces this lifecycle via commands such as `create-simulation`, `enter-simulation`, `simulation add/modify/exclude`, `list-simulations`, `summary <simulation>`, `apply-simulation`, and `discard-simulation` while the prompt indicates when the user is editing a simulation.

### `utils`

Utility helpers house cross-cutting concerns. Phase 0 ships the tracing bootstrapper (`init_tracing`) that configures an env-filtered subscriber with the crate defaulting to `info` level. Phase 2 introduces JSON persistence helpers that stage atomic writes and make loading/saving ledgers trivial for the CLI and future services.

### `errors`

`LedgerError` consolidates domain error reporting using `thiserror` for ergonomic conversions. IO and serialization errors are captured directly, and higher-level operations will extend the enum with contextual variants as functionality expands.

## Next Steps

1. Expand ledger operations with validated mutations that maintain account balances.
2. Introduce persistence adapters (file-based, SQLite) with serde-powered serialization.
3. Layer richer simulations and forecasting utilities, leveraging cached snapshots.
4. Wire additional tooling (benchmarks, fuzzing) as the codebase
   deepens.
