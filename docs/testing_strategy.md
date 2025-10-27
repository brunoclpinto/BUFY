# Testing Strategy — Phase 11 Snapshot

This note captures the current automated coverage before expanding reliability and stress tests.

## Existing Suites
- `cargo test` runs unit and integration modules covering budgeting math, persistence round-trips (`tests/persistence_suite.rs`), CLI script workflows (`tests/cli_script.rs`), recurrence scheduling (`tests/phase2_time_and_recurrence.rs`), currency formatting and FX tolerance (`tests/currency_tests.rs`), and simulation overlays (`tests/simulation_flow.rs`).
- `tests/ffi_integration.rs` exercises the dynamic shared library produced under the `ffi` feature, validating basic ledger lifecycle operations via the public C ABI.
- CLI and persistence suites validate backup rotation and recovery commands, ensuring atomic save logic works under normal conditions.
- Criterion benchmarks (`benches/performance.rs`) provide deterministic load/save, summary, and forecast workloads (`docs/performance.md` documents execution instructions).

## Recent Additions (Phase 11 — Step 2)
- Added `materialize_handles_backlog_across_multiple_periods` to confirm overdue recurrence materialization generates every missing instance and advances metadata.
- Added `simulation_exclusion_updates_budget_impact` to ensure simulation exclusions propagate through `summarize_simulation_in_window`.
- Added `valuation_policy_selects_expected_rate` to validate transaction-date vs. report-date FX valuation.
- Added `ffi_parallel_snapshots_are_thread_safe` to stress the shared ledger handle across threads via the snapshot API.
- Added `stress_repeated_save_load_and_forecast_cycles` to exercise repeated recurrence materialization, forecasting, simulation overlays, and persistence reload loops.
- Added `atomic_save_failure_preserves_original_file` to simulate an interrupted atomic save and verify the original ledger plus backups remain intact.

## Identified Gaps
- We still need extended-duration stress harnesses (multi-minute or hour scale) that run outside unit test timeouts, ideally via dedicated benchmarks or soak scripts.
- Fault-injection coverage should be expanded to cover truncated JSON payloads, backup restoration failures, and disk-full scenarios.
- Currency conversion coverage still needs rounding-mode assertions and disclosure provenance checks.
- CLI tests run in script mode but do not cover accessibility toggles (screen reader, high contrast) or FX import commands.

## Next Steps
- Implement dedicated reliability tests (unit + integration) targeting stress longevity, FX rounding/disclosure guarantees, CLI accessibility flows, and persistence fault recovery beyond atomic writes.
- Add stress fixtures and fault-injection utilities to validate persistence recovery guarantees.
- Extend documentation once the new suites are in place so contributors understand how to run and interpret the expanded coverage.
