# Testing Strategy — Phase 20

Phase 20 finalises the test plan for the CLI and core ledger library. The
coverage matrix below explains how automated suites, stress jobs, and manual
walkthroughs ensure every subsystem remains stable.

## Automated Suites

| Suite | Command | Focus |
| --- | --- | --- |
| `cargo test` | Unit + integration | Ledger math, recurrence, simulations, persistence round-trips, wizard validation, selection cancellation paths. |
| `cargo nextest run` | Parallel executor | Mirrors `cargo test` with improved reporting, enabling fast regression feedback in CI. |
| `cargo clippy --all-targets -- -D warnings` | Linting | Enforces style, derives, and accessibility helpers; guards against drift in output formatting. |
| `cargo fmt --all -- --check` | Formatting | Ensures consistent code style before release. |
| `cargo audit` | Supply-chain | Flags vulnerable dependencies; `atty` warnings are currently accepted because the transitive dependency (via `cbindgen`) has no maintained alternative. |

### Key Test Modules

- `tests/cli_script.rs` — End-to-end command pipelines covering ledger creation,
  account/category/transaction workflows, simulations, backups, and config
  toggles in script mode.
- `tests/persistence_suite.rs` — Save/load round-trips, atomic save failure
  injection, backup rotation and restore confirmation.
- `tests/currency_tests.rs` — Locale-aware formatting and accessibility
  aware negative-number rendering.
- `tests/phase2_time_and_recurrence.rs` — Recurrence projection, metadata
  refresh, and materialisation order.
- `tests/simulation_flow.rs` — Simulation add/modify/exclude/apply plus delta
  calculations.
- `tests/stress_suite.rs` — Long-running cycles of save → load → forecast to
  surface resource leaks or accumulation bugs.
- `src/cli/forms.rs` unit tests — Wizard success/cancel/back/help flows,
  validators, and recurrence editor defaults.

## Manual Walkthroughs

Manual verification complements automation for accessibility and UX:

1. **Golden path**: create ledger → add accounts/categories/transactions → save →
   backup → restore → summary.
2. **Simulation tour**: create simulation, enter mode, modify transactions,
   forecast with overlay, apply simulation, confirm ledger mutation.
3. **Accessibility**: toggle `config screen-reader on` and `config high-contrast on`,
   run `list`, `summary`, `forecast`, `transaction add`, ensuring prompts and
   output remain readable without colour.
4. **Error recovery**: provide invalid wizard input, cancel selections, attempt
   to restore a corrupted backup, simulate disk write failure (e.g. read-only
   temp directory), confirm CLI surfaces warnings without corrupting state.

Document the walkthrough outputs (or reference transcripts) whenever
regressions are fixed so they remain part of the release checklist.

## Release Checklist (Testing Portion)

- [x] `cargo fmt --all`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo test`
- [x] `cargo nextest run`
- [x] `cargo audit`
- [x] Manual walkthroughs recorded in release notes

Running this checklist is mandatory before tagging a release and updating the
CLI version metadata.
