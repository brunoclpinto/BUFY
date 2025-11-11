# Development Guide

This document supplements `docs/design_overview.md` with practical guidance for contributors working on Budget Core’s modular architecture.

## Module Layout

| Path | Purpose | See also |
| --- | --- | --- |
| `cli/` | Command definitions, `ShellContext`, selection providers, output/IO utilities | `src/cli/core.rs`, `src/cli/registry.rs` |
| `core/services/` | Business-logic helpers (`AccountService`, `CategoryService`, `TransactionService`, `SummaryService`) that mutate ledgers after validation | `src/domain` |
| `core/ledger_manager.rs` | Coordinates persistence and manages the in-memory ledger handle | `src/storage/json_backend.rs` |
| `domain/` + `ledger/` | Fundamental data structures (accounts, categories, transactions, recurrence/time utilities) with `Display` implementations for CLI output | `docs/design_overview.md` |
| `storage/` | Persistence backends (`JsonStorage`) and atomic save helpers | `config/mod.rs` |
| `config/` | `Config` + `ConfigManager`, backup/restore helpers, accessibility preferences | `cli/io.rs` |

Each module begins with a `//!` summary and public items have `///` doc comments referencing related modules via “See also” sections to keep Rustdoc cross-links navigable.

## Naming & Style

- Files use snake_case with one primary type per file (e.g., `account_service.rs`, `json_backend.rs`).
- Public enums implement `Display` so CLI output no longer needs to format `{:?}`.
- Manager methods use nouns for getters (`current_name`) and verbs for actions (`load`, `save_as`).
- Command handlers follow the `cmd_<entity>_<action>` pattern and are registered via `CommandRegistry`.
- All output goes through `cli::output`; direct `println!` calls are reserved for tests or very low-level debugging.
- `?` is preferred over `unwrap()` or `expect()` unless the panic conveys a meaningful invariant (locks poisoned, CLI misconfigured, etc.).

## Error Handling

- Domain/service functions return `ServiceResult<T>` or `BudgetError` enumerations.
- CLI layers convert errors into `CommandError`, which prints a failure followed by a `hint` message. Add actionable hints to new error paths so users see immediate recovery steps.
- Storage/config errors bubble up as `BudgetError::StorageError` or `ConfigError`; avoid stringly-typed messages in the CLI layer unless they include concrete recovery guidance.

## Testing Strategy

- `cargo test --all` runs unit + integration suites, script-mode checks, persistence/recurrence stress tests, and FFI loader checks.
- Prefer `tempfile::TempDir` for filesystem tests; the shared helper in `tests/common/mod.rs` keeps directories alive for the duration of each test run.
- When adding a CLI command, write a script-mode test in `tests/cli_script.rs` or a focused integration test in `tests/cli_tests.rs` so regressions show up quickly in CI.
- Benchmarks (`cargo bench`) use Criterion; keep them up to date when altering persistence or summary logic.

## Extending the CLI

1. Implement the handler in `cli/commands/<domain>.rs` using the `cmd_<entity>_<action>` naming pattern.
2. Register the handler inside `cli/commands/mod.rs::register_all` so it appears in completion/help output.
3. Use `ShellContext::with_ledger` / `with_ledger_mut` to safely access the current ledger, and route user feedback through `cli::output`.
4. Add documentation to `docs/user_guide.md` if the command changes user-visible workflows.

## Running Docs

`cargo doc --no-deps` generates the combined Rustdoc output. Every module emits a `//!` header and cross-links related layers via “See also” comments so contributors can navigate between CLI, services, and domain models without opening the source tree.
