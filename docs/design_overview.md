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

Phase 3 introduces an interactive `rustyline`-powered shell that wraps the ledger APIs with contextual menus and command dispatch. Highlights:

- **Ledger workflows** – `new-ledger`, `save`, `load`, and their named counterparts (`save-ledger`, `load-ledger`) keep the working set obvious. The prompt reflects the active ledger, and auto-complete/history improve ergonomics.
- **Data entry** – guided prompts for accounts, categories, and transactions with validation, plus script-mode (`BUDGET_CORE_CLI_SCRIPT=1`) automation for tests and CI. Transaction listings annotate recurrence hints (`[recurring]`, `[instance]`).
- **Recurrence tooling** – `recurring list/edit/clear/pause/resume/skip/sync` and `complete <idx>` manage schedules without leaving the shell.
- **Forecasting & simulations** – `forecast`, `summary <simulation>`, and `simulation add/modify/exclude` expose future-looking views side-by-side with base results.
- **Persistence integration** – the CLI auto-loads the last ledger, exposes backup/restore commands, and surfaces migration warnings emitted by `LedgerStore`.

### Simulations (Phase 5)

Simulations are persisted, name-addressable overlays that store only the delta relative to the authoritative ledger. Each simulation carries metadata (name, notes, timestamps, status) and a list of changes:

- `AddTransaction` — staged hypothetical entries.
- `ModifyTransaction` — partial patches against existing transactions.
- `ExcludeTransaction` — temporarily ignore a real transaction.

Ledger JSON now includes a `simulations` array so scenarios survive reloads and version bumps. The ledger exposes APIs to create, list, summarize, apply, and discard simulations, and budget summaries can optionally include a simulation overlay to show base/simulated totals plus deltas. The CLI surfaces this lifecycle via commands such as `create-simulation`, `enter-simulation`, `simulation add/modify/exclude`, `list-simulations`, `summary <simulation>`, `apply-simulation`, and `discard-simulation` while the prompt indicates when the user is editing a simulation.

### Recurrence & Forecasting (Phase 6)

Recurring schedules are encoded directly on `Transaction` records via a richer `Recurrence` definition (start date, interval, end condition, status, exceptions, metadata for last/next occurrences). Every recurrence owns a stable `series_id` so generated instances (pending ledger transactions) can be tied back to their definition without duplicating JSON structures. The `ledger::recurring` module walks each series iteratively, classifies instances as Overdue/Pending/Future, and produces:

- `RecurrenceSnapshot` summaries for quick CLI listings.
- `ForecastResult`/`ForecastReport` bundles that merge temporary projections into `BudgetSummary` outputs without mutating the authoritative ledger.
- `materialize_due_recurrences` helpers that clone scheduled occurrences into the ledger once a due date passes, preventing gaps between expected and real-world entries.

`Ledger::refresh_recurrence_metadata` keeps next-due hints persisted so reloads remain stable, while `forecast_window_report` composes forecasts with budgeting logic for any date window or simulation overlay. The CLI exposes an entire management surface via `recurring list/edit/clear/pause/resume/skip/sync`, the `forecast` command (with optional simulation overlays and custom ranges), and `complete <index>` for marking real activity. These operations keep recurrence data deterministic, detect conflicts, and protect against infinite projections by enforcing bounded windows. Older ledgers remain compatible—new metadata simply defaults when missing.

### `utils`

Utility helpers house cross-cutting concerns. Phase 0 ships the tracing bootstrapper (`init_tracing`) that configures an env-filtered subscriber with the crate defaulting to `info` level. Phase 2 introduces JSON persistence helpers that stage atomic writes and make loading/saving ledgers trivial for the CLI and future services. Phase 7 promotes this into a dedicated `LedgerStore` that:

- Resolves a stable home directory (`~/.budget_core` by default, overridable via `BUDGET_CORE_HOME`).
- Serializes ledgers deterministically (pretty JSON) while enforcing atomic writes through staged temp files.
- Maintains version metadata (`schema_version`) and invokes `Ledger::migrate_from_schema` plus recurrence refreshes on load.
- Creates timestamped `.json.bak` snapshots before overwriting existing ledgers and exposes backup/restore/list primitives with configurable retention.
- Tracks the “last opened ledger” so the CLI can resume stateful sessions automatically.

All persistence errors surface as `LedgerError::Persistence`, ensuring higher layers can present actionable guidance instead of panicking.

### Persistence (Phase 7)

- **File layout**: `ledger_name.json` files sit at the store root. Backups live under `backups/<ledger_name>/YYYY-MM-DDTHH-MM-SS.json.bak`. A `state.json` file records the last loaded ledger for convenience.
- **Atomic saves**: writes always target `<file>.tmp`, flush, and rename into place. Previous snapshots are retained before overwrites so users can roll back.
- **Schema evolution**: each save bumps `schema_version`. Loading older files runs migrations (e.g., refreshing recurrence metadata) and logs the steps taken so users know why a save is requested.
- **CLI integration**: `save-ledger`, `load-ledger`, `backup-ledger`, `list-backups`, and `restore-ledger` wrap the store APIs. Interactive sessions prompt for ledger names or allow custom paths via the legacy `save`/`load` commands. Script mode reuses the same infrastructure via environment variables to keep CI deterministic.
- **Recovery**: restore operations copy the chosen snapshot into place (optionally creating a safety backup first) and then reload the ledger, surfacing any validation warnings. Tests simulate interrupted saves to guarantee the original JSON is never corrupted.

### `errors`

`LedgerError` consolidates domain error reporting using `thiserror` for ergonomic conversions. IO and serialization errors are captured directly, and higher-level operations will extend the enum with contextual variants as functionality expands.

## Next Steps

1. Expand ledger operations with validated mutations that maintain account balances.
2. Introduce persistence adapters (file-based, SQLite) with serde-powered serialization.
3. Layer richer simulations and forecasting utilities, leveraging cached snapshots.
4. Extend persistence with alternate backends (SQLite/cloud) once JSON parity is battle-tested.
5. Wire additional tooling (benchmarks, fuzzing) as the codebase
   deepens.
