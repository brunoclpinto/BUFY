# Budget Core

Budget Core provides a reusable Rust toolkit for building budgeting workflows, simulations, and command-line experiences. Phase 0 establishes a reproducible development environment, initial domain models, and CI automation that future phases will build upon.

## Feature Highlights

- **Ledger fundamentals** – accounts, categories, transactions, and budget periods form the core domain (`src/ledger`).
- **Interactive CLI** – a `rustyline` REPL (`budget_core_cli`) with contextual prompts, script mode (`BUDGET_CORE_CLI_SCRIPT=1`), and rich command set for day‑to‑day operations.
- **Simulations** – name-addressable overlays that stage hypothetical changes without mutating the authoritative ledger.
- **Recurrence & forecasting** – recurring transactions, automatic schedule regeneration, and future projections with variance-aware summaries.
- **Currency-aware localization (Phase 8)** – base/reporting currency per ledger with locale-sensitive formatting and accessibility-aware output.
- **Managed persistence (Phase 7)** – deterministic JSON serialization under `~/.budget_core`, schema migrations, rotating backups, and recovery tooling.

For architectural background see `docs/design_overview.md`.

## Getting Started

1. **Install prerequisites**

   ```sh
   rustup toolchain install stable
   rustup component add clippy rustfmt rust-analyzer
   cargo install cargo-nextest cargo-audit cargo-edit
   ```

2. **Bootstrap the workspace**

   ```sh
   cargo fmt --all
   cargo build
   cargo test
   cargo nextest run
   cargo clippy --all-targets -- -D warnings
   cargo audit
   ```

3. **Launch the CLI**

   ```sh
   cargo run --bin budget_core_cli
   ```

   - The prompt indicates the active ledger (`ledger(demo) ⮞`). Use `help` or `help <command>` for inline docs.
   - Script mode (`BUDGET_CORE_CLI_SCRIPT=1`) accepts newline-delimited commands for tests/automation.
   - The CLI auto-loads the last opened ledger (tracked in `~/.budget_core/state.json`) when running interactively.

### CLI Quick Reference

| Area | Commands | Notes |
| --- | --- | --- |
| Ledger lifecycle | `new-ledger`, `load [path]`, `save [path]`, `load-ledger <name>`, `save-ledger [name]` | Named saves use the managed store; path-based commands operate on arbitrary JSON files. |
| Persistence tooling | `backup-ledger [name]`, `list-backups [name]`, `restore-ledger <idx|pattern> [name]` | Snapshots live under `~/.budget_core/backups/<name>/YYYY-MM-DDTHH-MM-SS.json.bak`. |
| Currency & locale | `config [show|base-currency|locale|negative-style|screen-reader|high-contrast|valuation]` | Persisted per-ledger settings for currency display, locale defaults, valuation policies, and accessibility. |
| Data entry | `add account`, `add category`, `add transaction`, `list [accounts|categories|transactions]` | Transaction list output shows recurrence hints (`[recurring]`, `[instance]`). |
| Recurrence | `recurring list/edit/clear/pause/resume/skip/sync`, `complete <idx>` | Schedules track start/end dates, exceptions, and automatically materialize overdue instances. |
| Forecasting | `forecast [simulation] [<n> <unit> | custom <start> <end>]` | Produces future inflow/outflow projections plus window-specific budget summaries. |
| Simulations | `create-simulation`, `enter-simulation`, `simulation add/modify/exclude`, `list-simulations`, `summary <simulation>`, `apply-simulation`, `discard-simulation` | Enables “what-if” comparisons against the base ledger. |
| Summaries | `summary [past|future <n> | custom <start> <end>]` | Default view shows the active budget period; optional simulation overlay highlights deltas. |
| Meta | `version` | Print build metadata (crate version, git hash, target, rustc, FFI version when available). |

Use `BUDGET_CORE_HOME=/custom/path` to relocate the managed store. `save-ledger <name>` remembers the canonical filename and enables quick resaves without re-entering the path.

## Forecasting & Recurrence

Budget Core now understands recurring obligations and can materialize missed occurrences automatically:

- `recurring list [overdue|pending|all]` surfaces every recurrence with next-due dates, overdue counts, and status (Active/Paused/Completed). Use `recurring edit <transaction_index>` to attach or update a schedule for any transaction, `recurring clear` to remove it, `recurring pause`/`recurring resume` to toggle activity, `recurring skip <index> <YYYY-MM-DD>` to add exceptions, and `recurring sync [YYYY-MM-DD]` to backfill overdue ledger entries.
- `forecast [simulation_name] [<number> <unit> | custom <start> <end>]` produces a deterministic projection for the requested window and reports inflow/outflow totals, overdue vs. pending counts, and the top upcoming instances. Prefix the command with a simulation name to preview "what-if" schedules.
- `complete <transaction_index> <YYYY-MM-DD> <amount>` marks a scheduled transaction as finished and updates recurrence metadata automatically.

Recurrence state is persisted with the ledger JSON so restarting the CLI preserves start dates, next occurrences, and skipped dates. Use `recurring sync` after structural changes (new accounts/categories) to ensure schedules stay aligned.

## Persistence & Backups

Phase 7 introduces a fully managed JSON store rooted at `~/.budget_core` (override with `BUDGET_CORE_HOME`). Each ledger is saved as `<name>.json`, accompanied by a rotating set of timestamped backups under `backups/<name>/YYYY-MM-DDTHH-MM-SS.json.bak`.

- `save-ledger [name]` writes the in-memory ledger using atomic temp-file swaps and records the name for future quick saves.
- `load-ledger <name>` retrieves a named ledger while validating schema versions, rebuilding recurrence metadata, and surfacing any migration warnings.
- `backup-ledger [name]` snapshots the current file, `list-backups [name]` enumerates available restore points, and `restore-ledger <index|pattern> [name]` reverts to the desired snapshot (with interactive confirmation).
- The classic `save [path]` / `load [path]` commands remain for ad-hoc JSON paths.

All saves are deterministic (bar timestamps), schema versioned, and guard against corruption via temp files plus optional rolling backups. A small state file remembers the last open ledger; interactive sessions auto-load it on startup for continuity.

Additional architectural notes are captured in `docs/design_overview.md`.

### File Layout

```
~/.budget_core/
├── demo.json                 # canonical ledger save
├── backups/
│   └── demo/
│       ├── 2025-10-26T08-20-00.json.bak
│       └── …
└── state.json                # remembers the last opened ledger
```

Every save produces pretty JSON for human inspection, and backups are pruned according to the store’s retention setting (default 5). Loads validate schema versions (`schema_version`), rebuild recurrence metadata, and surface migration notes in the CLI.

## Currency & Localization

- **Base currency / valuation policy** – Summaries assume all amounts are recorded in the ledger’s base currency. `config valuation <transaction|report|custom>` controls the date referenced in disclosure footers but no longer triggers FX conversions.
- **Account/transaction currency** – Account creation prompts for a currency override; transactions inherit from their source account unless explicitly set. Mixing currencies without manual conversion will mark summaries as incomplete.
- **Locale & formatting** – `config locale <tag>` adjusts decimal/grouping separators, date formats, and the first weekday; `config negative-style`, `config screen-reader`, and `config high-contrast` tune CLI output for accessibility.
- **Disclosures** – Budget summaries and forecasts include a footer noting the active valuation policy and reminder that FX conversion is unavailable.

### Accessibility & Internationalization

- **Screen reader mode** (`config screen-reader on`) replaces ambiguous glyphs with explicit words (for example, `-` becomes “minus”) and ensures tables are narratable top-to-bottom. Disable again with `config screen-reader off`.
- **High-contrast mode** (`config high-contrast on`) removes ANSI color and emoji reliance so totals remain legible in monochrome terminals. It is safe to enable alongside screen reader mode.
- **Locale fallbacks** – When an unsupported `language_tag` is provided, the CLI keeps the requested tag but emits a warning while reverting to default separators. Adjust the decimal or grouping character manually with `config locale`.
- **Date formatting** – Locale changes also update the first weekday so weekly/monthly summaries align with regional expectations. Use `config locale en-GB` (Monday week start) vs. `en-US` (Sunday).
- See `docs/localization_and_accessibility.md` for deeper guidance on translation, formatting rules, and output conventions.

## Development Conventions

- Crate edition: Rust 2021.
- Tracing is initialized via `budget_core::init()` or `budget_core::utils::init_tracing()`.
- All warnings are denied by default (`.cargo/config.toml`), so fix lint issues before committing.

## Testing & CI

- `cargo test` exercises unit, integration, CLI-script, currency formatting, and persistence suites.
- `cargo test --features ffi` repeats the suite against the shared library to ensure ABI stability.
- `cargo nextest run` and `cargo clippy --all-targets -- -D warnings` keep execution fast in CI.
- CLI scenarios in `tests/cli_script.rs` demonstrate script mode pipelines; `tests/persistence_suite.rs` guards backup/restore flows.
- `cargo test --test stress_suite` runs the soak test that iterates through recurrence materialization, simulations, forecasts, and save/load cycles.
- `cargo bench` runs Criterion benchmarks (see `docs/performance.md`) for load/save, summary, and forecast workloads. Generated reports live under `target/criterion`.

## Troubleshooting

- **Schema migrations** – After upgrading, run `load-ledger <name>`; migration notes are echoed in the CLI. If a load fails, restore from `list-backups` and inspect the JSON diff.
- **Currency mismatches** – When summaries warn about incomplete conversions, align the ledger/accounts on a single currency or adjust transaction amounts manually; automatic FX is not supported.
- **Atomic save failures** – Errors mentioning temp files indicate filesystem permissions or disk space issues. Verify write access to `~/.budget_core` (or set `BUDGET_CORE_HOME`) and rerun `save-ledger`.
- **Recurrence drift** – Run `recurring sync <YYYY-MM-DD>` to backfill overdue instances before generating forecasts. The command emits a summary of newly materialized transactions.
- **Accessibility output** – If screen readers skip totals, ensure `config screen-reader on` is set; the CLI will re-render with explicit wording.
- Additional developer-focused notes, schema references, and integration steps for Swift/Kotlin/C# live in `docs/design_overview.md` and `docs/integration_guides.md`.

## License

Licensed under either of

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT license (`LICENSE-MIT`)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project is licensed under the same dual license terms.
