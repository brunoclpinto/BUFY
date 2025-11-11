# Budget Core · v2.0 “Modular Refactor”

Budget Core is a reusable Rust toolkit for building budgeting workflows, simulations, and accessible command-line experiences.
Version 2.0 completes the modular refactor, cleanly separating the CLI, service layer, domain models, and persistence components.

```
┌──────────────────────────────────────────────────────┐
│ CLI Layer (cli/)                                     │
│   • CommandRegistry and commands/* handlers          │
│   • ShellContext, selection providers, IO utilities  │
└──────────────────────────────┬───────────────────────┘
                               ↓
┌──────────────────────────────────────────────────────┐
│ Core Services (core/services/, cli/core.rs)          │
│   • Account/Category/Transaction/Summary logic       │
│   • LedgerManager orchestration                      │
└──────────────────────────────┬───────────────────────┘
                               ↓
┌──────────────────────────────────────────────────────┐
│ Domain Models (domain/, ledger/)                     │
│   • Account/Category/Ledger/Simulation types         │
│   • Recurrence + time utilities                      │
└──────────────────────────────┬───────────────────────┘
                               ↓
┌──────────────────────────────────────────────────────┐
│ Storage & Config (storage/, config/)                 │
│   • JsonStorage, ConfigManager, backup helpers       │
└──────────────────────────────────────────────────────┘
```

## Feature Highlights

- **Ledger fundamentals** – accounts, categories, transactions, and budget periods form the core domain.
- **Interactive CLI** – `budget_core_cli` provides a `rustyline` REPL with contextual prompts, script mode (`BUDGET_CORE_CLI_SCRIPT=1`), and a centralized command registry.
- **Simulations** – name-addressable overlays stage hypothetical changes without touching the authoritative ledger.
- **Recurrence & forecasting** – recurring transactions, automatic schedule regeneration, and future projections with variance-aware summaries.
- **Localization & accessibility** – locale-sensitive formatting, plain mode, screen-reader/high-contrast switches, and optional audio feedback cues.
- **Managed persistence** – deterministic JSON serialization under `~/.budget_core`, schema migrations, rotating backups, and recovery tooling.

### Documentation

- Architecture design notes: `docs/design_overview.md`
- Developer guide: `docs/development.md`
- User guide: `docs/user_guide.md`
- Localization & accessibility: `docs/localization_and_accessibility.md`
- Testing plan: `docs/testing_strategy.md`

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
   - Script mode (`BUDGET_CORE_CLI_SCRIPT=1`) accepts newline-delimited commands for automation.
   - The CLI auto-loads the last opened ledger (tracked in `~/.budget_core/state.json`) when running interactively.

### CLI Quick Reference

```
ledger new Household
account add
transaction edit
```

| Area | Commands | Notes |
| --- | --- | --- |
| Ledger lifecycle | `new-ledger`, `load [path]`, `save [path]`, `load-ledger <name>`, `save-ledger [name]` | Named saves use the managed store; path-based commands operate on arbitrary JSON files. |
| Persistence tooling | `backup-ledger [name]`, `list-backups [name]`, `restore-ledger <idx|pattern> [name]` | Snapshots live under `~/.budget_core/backups/<slug>/<slug>_YYYYMMDD_HHMM[_note].json`. |
| Config management | `config show`, `config set <locale|currency|theme|last_opened_ledger> <value>`, `config audio-feedback <on|off>`, `config backup [note]`, `config backups`, `config restore [name]` | Preferences live in `~/.budget_core/config/config.json` with backups under `config/backups/`. |
| Data entry | `transaction add/edit/remove/show/complete`, `account add/edit/list`, `category add/edit/list`, `list [accounts|categories|transactions]` | List commands now render consistent tables respecting locale/currency. |
| Recurrence | `recurring list/edit/clear/pause/resume/skip/sync`, `complete <idx>` | Schedules track start/end dates, exceptions, and automatically materialize overdue instances. |
| Forecasting | `forecast [simulation] [<n> <unit> | custom <start> <end>]` | Produces future inflow/outflow projections plus window-specific budget summaries. |
| Simulations | `create-simulation`, `enter-simulation`, `simulation add/modify/exclude`, `list-simulations`, `summary <simulation>`, `apply-simulation`, `discard-simulation` | Enables what-if comparisons against the base ledger. |
| Summaries | `summary [past|future <n> | custom <start> <end>]` | Default view shows the active budget period; optional simulation overlay highlights deltas. |
| Meta | `version` | Prints build metadata (crate version, git hash, target, rustc, FFI version when available). |

#### CLI Output & Accessibility

- All output flows through `cli::output`, which attaches explicit labels (`INFO`, `SUCCESS`, `WARNING`, `ERROR`, `HINT`) plus emoji/colour decorations. `config theme plain` or `config high-contrast on` disables colour/emoji while keeping labels intact for screen readers.
- List commands render monospace tables with Unicode borders when colour is enabled and ASCII borders in plain mode so screen readers can enumerate columns.
- `config screen-reader on` switches to sentence-form rows, while `config audio-feedback on` adds soft beeps to warnings/errors for low-vision cues.
- Interactive lists show numbered rows with `[default]` hints and `Type cancel to abort.` prompts; script mode bypasses the prompts entirely.
- `BUDGET_CORE_CLI_SCRIPT=1` disables interactive prompts/line-editing so deterministic scripts can feed newline-delimited command files (see `tests/cli_script.rs`).

## Config, Simulation, & Forecast Capabilities

- **Config** – `config show` prints locale/currency/theme/last-ledger information plus ledger-format details when a ledger is loaded. `config set`, `config screen-reader`, `config high-contrast`, and `config audio-feedback` update preferences live. Backups are versioned and restorable via `config backup|restore`.
- **Simulations** – `create-simulation`, `enter-simulation`, `simulation add/modify/exclude`, `apply-simulation`, and `discard-simulation` make modelling scenarios trivial. `summary <name>` and `forecast <name>` overlay simulation deltas on the base ledger.
- **Forecasts** – `forecast [simulation] [<n> <unit> | custom <start> <end>]` renders inflow/outflow projections in the same table style. Summaries and forecasts honour locale, base currency, and valuation policy.

See `docs/user_guide.md` for step-by-step workflows and `docs/development.md` for extending the CLI/service layers.

## Currency & Localization

- **Base currency / valuation policy** – Summaries assume all amounts are recorded in the ledger’s base currency. `config valuation <transaction|report|custom>` controls the date referenced in disclosure footers.
- **Locale & formatting** – `config locale <tag>` adjusts decimal/grouping separators, date formats, and the first weekday. `config negative-style`, `config screen-reader`, and `config high-contrast` tune CLI output for accessibility.
- **Disclosures** – Budget summaries and forecasts include a footer noting the active valuation policy and reminder that FX conversion is unavailable.
- Refer to `docs/localization_and_accessibility.md` for translation and formatting guidance.

## Development Conventions

- Crate edition: Rust 2021.
- Tracing is initialized via `budget_core::init()` or `budget_core::utils::init_tracing()`.
- All warnings are denied by default (`.cargo/config.toml`), so fix lint issues before committing.
- Module naming uses snake_case files with one entity per file; enums implement `Display` so CLI output remains readable. `docs/development.md` covers naming, testing, and error-handling expectations in detail.

## Testing & CI

- `cargo test --all` exercises unit, integration, CLI-script, currency formatting, and persistence suites.
- `cargo nextest run` executes the same tests in parallel; `cargo clippy --all-targets -- -D warnings` keeps lint debt at zero.
- `cargo test --features ffi` loads the shared library dynamically to guarantee ABI stability.
- CLI scenarios in `tests/cli_script.rs` demonstrate script mode pipelines; `tests/persistence_suite.rs` guards backup/restore flows, and `tests/stress_suite.rs` performs long-running save/load/forecast loops.
- `cargo bench` runs Criterion benchmarks (see `docs/performance.md`) for load/save, summary, and forecast workloads. Generated HTML reports live under `target/criterion`.

## License

Dual licensed under MIT or Apache 2.0 as documented in `LICENSE` and `LICENSE-APACHE`. See the individual files for more details.
