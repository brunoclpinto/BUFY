# Budget Core — Design Overview

Budget Core is the domain foundation for a multi-surface budgeting experience. The project starts as a reusable Rust library with a thin CLI harness and will grow to power rich simulations, data ingestion, and reporting workflows. This document introduces the core modules planned for Phase 0 and the guiding principles for future development.

## Architectural Principles

- **Deterministic & auditable** – every API is designed to be side-effect free unless explicitly mutating state. JSON persistence is formatted and ordered to make diffs meaningful, and atomic saves prevent partially-written ledgers.
- **Composable domain primitives** – accounts, categories, transactions, budgets, recurrences, and simulations live in dedicated modules so higher-level workflows (CLI, automation) can mix/match them without tight coupling.
- **Extensible simulations & forecasting** – simulations operate on deltas while recurrence/forecasting logic is pure and serializable. This keeps “what-if” flows hermetic, but ready for richer surfaces (GUI, services) later.
- **Observability-first** – `budget_core::init()` wires tracing before any other work, and CLI/reporting surfaces bubble warnings (schema migrations, orphaned transactions) directly to the user instead of burying them in logs.
- **Resilient persistence** – JSON files are schema versioned, backups are automatic, migrations are explicit, and recovery commands are first-class citizens of the CLI. Our goal is to make “data loss” an impossible class of bug.

## Module Breakdown

### `ledger`

The ledger module owns the core domain types. Each struct is `serde`-friendly and intentionally “dumb” so that advanced behavior can be layered on top without mutating the raw data.

| Type | Purpose | Key Fields | Design Rationale |
| --- | --- | --- | --- |
| `Account` | Represents an asset, liability, or logical bucket. | `id`, `name`, `kind` (`AccountKind`). | IDs are UUIDs for cross-file stability; `kind` drives reporting semantics but does not strictly enforce debit/credit rules, keeping the domain flexible. |
| `Category` | Labels transactions for budgeting and reporting. | `id`, `name`, `kind` (`CategoryKind`), optional parent. | Nested parents let us do future roll-ups; `Option<Uuid>` avoids enforcing hierarchies before the UI needs them. |
| `Budget` | Expresses recurring spending guardrails. | `limit_amount`, `recurrence: TimeInterval`, `is_active`. | Reuses the same interval math as transactions so schedules stay consistent. |
| `TimeInterval` / `TimeUnit` | Date math helper with “next/previous/add” APIs. | `every`, `unit`. | Keeps period calculations centralized and testable, including tricky month/year shifts and leap days. |
| `Transaction` | Authoritative record of budgeted vs. real movement. | `from_account`, `to_account`, `category_id`, `scheduled_date`, `actual_*`, `recurrence`, `recurrence_series_id`, `status`. | Bundling recurrence metadata with the template transaction removes the need for a separate recurrence table and keeps JSON intuitive (“this transaction repeats monthly…”). `recurrence_series_id` decouples generated instances from their template while maintaining referential integrity. |
| `Recurrence` | Scheduling rule and derived metadata. | `series_id`, `start_date`, `interval`, `mode`, `end`, `exceptions`, `status`, `last_generated`, `next_scheduled`. | Storing derived metadata (like `next_scheduled`) reduces recomputation cost and gives the CLI instant answers. Metadata is refreshed whenever transactions mutate or after deserialization. |
| `Ledger` | Aggregates all above data plus simulations and timestamps. | `budget_period`, collections of accounts/categories/transactions/simulations, `schema_version`, `created_at`/`updated_at`. | The ledger struct is the sole persistence surface. All CLI commands operate on it via immutable or mutable references, which keeps ownership clear and reduces accidental mutation. |

Budget summaries (Phase 4) and recurrence projection helpers (Phase 6) live alongside the core types so they can be reused by both CLI and future API layers without duplication.

### `simulation`

Simulations are modeled as change sets on top of a ledger snapshot:

- `Simulation` captures metadata (name, notes, timestamps, status).
- `SimulationChange` enumerates add/modify/exclude operations.
- `ledger::Ledger::transactions_with_simulation` materializes an “overlay” list to feed budgeting/reporting APIs, while `apply_simulation` rewrites the canonical ledger when the user commits the plan.

Design trade-offs:

- **Delta-based** rather than full copies to minimize JSON size and make intent obvious (“this simulation modifies txn X”).
- **Pending-only edits** – once applied or discarded, the simulation is still serialized for auditability but marked non-editable, preventing accidental replays.

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

The `ledger::recurring` module isolates all scheduling math. Flow:

1. **Authoring** – when a transaction is created with recurrence info, `Transaction::set_recurrence` stamps a `series_id` (defaulting to the transaction UUID) and ensures `start_date` matches the template’s `scheduled_date`. This keeps JSON human-readable.
2. **Metadata refresh** – after any mutation or deserialization, `Ledger::refresh_recurrence_metadata` rebuilds:
   - occurrence counts,
   - last generated/completed dates,
   - next due date (skipping exceptions and respecting `RecurrenceEnd`).
   The metadata is persisted to accelerate CLI calls and provide meaningful hints even before recomputation.
3. **Materialization** – `materialize_due_recurrences` scans series and creates concrete transactions for overdue occurrences that lack ledger entries. This supports “auto-posting” once a period begins without waiting for manual entry.
4. **Forecasting** – `forecast_for_window` walks occurrences inside a `DateWindow`, producing:
   - existing but incomplete instances (pending/overdue),
   - synthetic “future” entries that feed budget summaries without mutating the ledger.
   The CLI uses this to show inflow/outflow projections, counts of overdue/pending items, and top-line financial deltas.

Decision rationale:

- Keeping recurrence definitions inline with transactions keeps the JSON approachable (“a monthly rent template”).
- `RecurrenceMode` (“FixedSchedule”, “AfterLastPerformed”) supports both hard due dates and “repeat N days after last completion”, covering common workflows.
- Exceptions are modeled as concrete dates to guarantee reproducibility, and we log warnings if a rule becomes ambiguous (e.g., end condition reached but still active).

### `utils`

Utility helpers house cross-cutting concerns.

- `budget_core::utils::init_tracing` sets up an `EnvFilter`-driven `tracing` subscriber so both CLI and tests get consistent logging.
- `budget_core::utils::persistence::LedgerStore` (Phase 7) is the single entry point for persistence. Responsibilities:
  - Resolve the base directory (`~/.budget_core` or `BUDGET_CORE_HOME`).
  - Generate canonical filenames (slugified ledger names), temp-file paths, and backup directories.
  - Perform deterministic, pretty JSON serialization (`serde_json::to_string_pretty`) and atomic writes via `<file>.tmp` + `rename`.
  - Run schema migrations by calling `Ledger::migrate_from_schema` and `refresh_recurrence_metadata` on load, recording any warnings.
  - Maintain `state.json` so the CLI can auto-load the previous ledger.
  - Manage retention-limited backups (`YYYY-MM-DDTHH-MM-SS.json.bak`) and expose `backup_named`, `list_backups`, and `restore_backup` APIs.

Error handling:

- IO / JSON issues bubble up via `LedgerError::Persistence`.
- Validation helpers warn about orphaned transactions, missing accounts, or inactive recurrences without silently mutating data.

### Persistence (Phase 7) – Data & Process Lifecycle

1. **Save**
   - CLI mutates the in-memory `Ledger`.
   - `LedgerStore::save_named` or `save_to_path` clones the ledger (immutability ensures we don’t partially mutate state if a write fails), serializes to pretty JSON, writes `<file>.tmp`, and atomically renames to the final path.
   - If an existing file is being overwritten, a `.json.bak` snapshot is created first and old backups are pruned to respect retention.
   - `schema_version` is updated and `updated_at` re-stamped.
2. **Load**
   - `LedgerStore::load_named` reads JSON, deserializes into a `Ledger`, runs migrations, refreshes recurrence metadata, and validates references (issues are surfaced as CLI warnings).
   - The CLI records the ledger name/path, resets active simulations, and updates `state.json`.
3. **Backup / Restore**
   - `backup-ledger` is an alias for `LedgerStore::backup_named`, giving the user an explicit restore point.
   - `restore-ledger` copies the selected snapshot into place (also backing up the current file for safety) and immediately reloads it so the user sees the resulting warnings/migrations.
4. **Script mode**
   - Scripted flows (tests or automation) often chain commands such as `new-ledger Demo monthly` / `save-ledger demo`. Because script mode runs non-interactively, all prompts fall back to defaults or require explicit arguments.

The combination of atomic writes, JSON readability, migration hooks, and CLI feedback ensures we can evolve the schema without breaking older ledgers or requiring manual interventions.

### JSON Schema Reference

Persisted ledgers are deterministic, pretty-printed JSON documents. Every save includes the following top-level structure:

| Key | Type | Notes |
| --- | --- | --- |
| `id` | UUID string | Stable ledger identifier (never regenerated). |
| `name` | String | Human-friendly ledger label. |
| `budget_period` | Object `{ "every": u32, "unit": "Day\\|Week\\|Month\\|Year" }` | Drives reporting window calculations. |
| `base_currency` | ISO 4217 code (string) | Reporting currency; defaults to `USD`. |
| `locale` | Object `{ language_tag, decimal_separator, grouping_separator, date_format, first_weekday }` | Determines formatting defaults. |
| `format` | Object `{ currency_display, negative_style, screen_reader_mode?, high_contrast_mode? }` | Optional; omitted when matches defaults. |
| `valuation_policy` | Object `{ "kind": "transaction_date\\|report_date", "custom_date"?: "YYYY-MM-DD" }` | Policy name plus optional explicit date. |
| `accounts` | Array of `Account` | Each entry contains `id`, `name`, `kind`, optional `category_id`, optional `currency`. |
| `categories` | Array of `Category` | Each entry includes `id`, `name`, `kind`, optional `parent_id`. |
| `transactions` | Array of `Transaction` | Fields include `id`, `from_account`, `to_account`, `category_id?`, `scheduled_date`, `actual_date?`, `budgeted_amount`, `actual_amount?`, `currency?`, `status`, `recurrence?`, `recurrence_series_id?`. |
| `simulations` | Array of `Simulation` | Contains metadata (`status`, timestamps) and a list of `SimulationChange` deltas. |
| `created_at` / `updated_at` | RFC3339 timestamps | Always recorded in UTC. |
| `schema_version` | Integer | Used by migrations to determine upgrade steps. |

The `Transaction.recurrence` object mirrors the in-memory type:

```json
{
  "series_id": "UUID",
  "start_date": "2025-01-01",
  "interval": { "every": 1, "unit": "Month" },
  "mode": "FixedSchedule",
  "end": { "type": "Never" },
  "exceptions": ["2025-02-01"],
  "status": "Active",
  "last_generated": "2025-03-01",
  "last_completed": "2025-03-01",
  "generated_occurrences": 2,
  "next_scheduled": "2025-04-01"
}
```

Unknown keys are preserved during round-trips so future schema versions can add fields without breaking older binaries. When `Ledger::migrate_from_schema` encounters missing values it initializes them with sensible defaults (e.g., populating `base_currency` or `locale` for legacy files).

### Data Lifecycle & Decision Rationale

| Stage | Description | Why it matters |
| --- | --- | --- |
| Authoring | Users create ledgers, accounts, categories, and transactions via CLI. Script mode allows fixtures/tests to do the same. | Keeps all state mutations explicit and traceable; no hidden side effects. |
| Scheduling | Recurrence definitions live inside transactions, and metadata is auto-refreshed. | Embedding the schedule with its template keeps files self-documenting and avoids separate recurrence tables. |
| Forecasting | For any window, the ledger merges planned, pending, and projected transactions into a `ForecastReport`. | Budget summaries remain deterministic and inspectable without mutating the ledger. |
| Persistence | Saves always go through `LedgerStore`, ensuring atomic writes + backups + schema versioning. | Prevents corruption and provides a single seam for future storage backends (e.g., SQLite/cloud). |
| Recovery | Built-in CLI commands discover backups, restore snapshots, and surface warnings/migrations. | Users don’t need to leave the shell or manually manipulate files to recover from mistakes. |

Key decisions:

- **JSON format** – chosen for transparency; users can open ledger files in any editor, diff them in git, and make surgical fixes if necessary. Pretty printing trades slightly larger files for readability.
- **UUID identifiers** – guarantee uniqueness across merges/backups and allow us to regenerate derived structures without losing referential integrity.
- **Atomic saves** – `rename` is an atomic operation on POSIX/Windows, ensuring either the old file or the new one exists, never a half-written hybrid.
- **Backups-per-save** – providing a rolling history removes the need for a separate “snapshot” command before risky operations and makes restore flows trivial.
- **CLI-first UX** – building the feature set inside the CLI keeps the surface area small while making sure every workflow (interactive or automated) can exercise the same APIs.

### Currency & Localization (Phase 8)

- **Configuration model**:
  - `CurrencyCode` and `FormatOptions` live on the ledger, while accounts/transactions store optional overrides so original units are preserved in JSON.
  - `LocaleConfig` drives number/date formatting plus the first weekday, keeping summaries aligned with local budgeting norms.
  - `ValuationPolicy` (transaction date, report date, or explicit custom date) is evaluated through a `ConversionContext` passed to every aggregate. With FX disabled, the policy only influences disclosure messaging.
- **Aggregation & disclosure**:
  - `Ledger::convert_amount` now assumes ledger and transaction currencies match; mismatches raise `LedgerError::InvalidInput` so consumers can handle the failure explicitly.
  - Successful conversions still emit parity disclosures (“base currency parity”) so reports remain auditable.
- **Localization & accessibility**:
  - `format_currency_value` honors locale separators, currency style, and negative-style preferences while screen-reader mode replaces ambiguous symbols with readable phrases.
  - High-contrast mode disables ANSI color usage; warning prefixes automatically switch from emoji to text when assistive modes are enabled.
- **CLI controls**:
  - `config base-currency|locale|negative-style|screen-reader|high-contrast|valuation` persists preferences.
  - Transaction listings, summaries, forecasts, and simulations consume these settings automatically.

### Testing & Quality Infrastructure

- **Unit & integration tests** – `cargo test` executes suites covering budgeting math, recurrence scheduling, currency conversions, CLI script flows, persistence, simulations, and the FFI boundary. Tests are organized by concern (`tests/phase2_time_and_recurrence.rs`, `tests/currency_tests.rs`, etc.).
- **Stress harness** – `tests/stress_suite.rs` simulates months of ledger activity, repeatedly running recurrence materialization, forecasts, simulations, and persistence save/load cycles to guard against drift and IO regressions.
- **Fault injection** – `tests/persistence_suite.rs::atomic_save_failure_preserves_original_file` validates that atomic saves never corrupt the primary ledger when temp-file creation fails; additional recovery scenarios are slated for future expansion.
- **FFI regression** – `tests/ffi_integration.rs` dynamically loads the shared library, exercises thread-safe snapshotting, and verifies error propagation for invalid inputs.
- **Benchmarks** – `cargo bench` (Criterion) covers load/save, summary generation, and forecasting. Results are documented in `docs/performance.md`, and each run emits HTML reports under `target/criterion`.
- **Coverage policy** – new features are expected to land with tests that prove happy-path behavior plus relevant edge cases. Stress and benchmark suites should be extended when features affect performance-sensitive paths (recurrence, persistence, currency handling). See `docs/testing_strategy.md` for the current suite inventory and future reliability targets.

### CLI Usage: Interactive vs. Script

| Workflow | Interactive Mode | Script Mode |
| --- | --- | --- |
| Start session | `cargo run --bin budget_core_cli` (auto-load last ledger). | `BUDGET_CORE_CLI_SCRIPT=1 cargo run --bin budget_core_cli -- < commands.txt` |
| Create ledger | `new-ledger Demo monthly` (prompts for name/period if omitted). | `new-ledger Demo every 2 weeks` |
| Work with recurrences | `recurring edit 3`, `recurring list overdue`, `complete 5 2025-02-01 1200`. | scripted commands: `recurring sync 2025-03-01` etc. |
| Save/backup | `save-ledger household`, `backup-ledger`. | `save-ledger household` (no prompts). |
| Restore | CLI asks for confirmation and reloads automatically. | `restore-ledger 0 household` (non-interactive; assumes the reference is either index or substring). |

Script mode is deterministic: prompts are disabled, so each command must provide all required arguments. This keeps CI fixtures repeatable (see `tests/cli_script.rs`).

### FFI Bridge (Phase 9)

- **Goal**: expose the same ledger API to Swift, Kotlin, and C# via a stable C ABI. Rust remains the single source of truth; bindings only marshal data.
- **Module structure**: the `ffi` feature (see `src/ffi/mod.rs`) hosts version identifiers (`CORE_VERSION`, `FFI_VERSION`), error categories, and forthcoming opaque handles (`LedgerHandle`, `ResultHandle`). The full API groups (ledger, accounts, transactions, reports, simulations, settings) are described in `docs/ffi_spec.md`.
- **Versioning**: bindings must check both version strings at initialization to ensure compatibility. FFI version bumps only occur when the ABI changes, while core version bumps follow business logic changes.
- **Error handling**: every exported function returns an integer code (see `FfiErrorCategory`). Bindings map these into their native Result/Exception mechanisms.
- **Thread safety**: handles will wrap `Arc<Mutex<_>>` so that GUI threads can call into the core concurrently without data races.
- **Integration testing**: `cargo test --features ffi` now runs dynamic-loading tests (`tests/ffi_integration.rs`) that mimic a foreign client by invoking the compiled shared library through `libloading`. Future language bindings should provide equivalent harnesses.

A detailed API blueprint, memory-ownership rules, and serialization expectations live in `docs/ffi_spec.md`, while platform-specific import guidance is captured in `docs/integration_guides.md`. Later implementation steps (Phase 9.2+) will flesh out the actual exported functions, generate language-specific bindings, and wire CI to publish the resulting artifacts.

### `errors`

`LedgerError` is the single error enum exposed by domain/persistence APIs. Major variants:

- `Io`, `Serde` – low-level issues; surfaced directly in CLI so users know whether it’s a permissions problem or malformed JSON.
- `InvalidRef`, `InvalidInput` – domain validation failures (unknown IDs, empty names, etc.).
- `Persistence` – wraps higher-level issues (backup missing, store misconfigured).

CLI helpers map these into `CommandError`, allowing interactive sessions to provide guidance (“use `save` first”, “ledger not loaded”) while script mode propagates the failure to stdout/stderr for tests to assert against.

## Next Steps

1. Expand ledger operations with validated mutations that maintain account balances.
2. Introduce persistence adapters (file-based, SQLite) with serde-powered serialization.
3. Layer richer simulations and forecasting utilities, leveraging cached snapshots.
4. Extend persistence with alternate backends (SQLite/cloud) once JSON parity is battle-tested.
5. Wire additional tooling (benchmarks, fuzzing) as the codebase
   deepens.
6. Phase 9 follow-ups: implement the FFI modules per `docs/ffi_spec.md`, generate bindings, and expand cross-platform test coverage.
7. Phase 11 performance tracking: maintain the Criterion harness (`benches/performance.rs`) and record benchmark results (see `docs/performance.md`).
