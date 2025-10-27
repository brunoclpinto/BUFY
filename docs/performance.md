# Performance & Benchmark Guide

Phase 11 introduces a reproducible benchmark harness so we can track and optimise the budgeting core under realistic workloads.

## Running the Benchmarks

Benchmarks use the [`criterion`](https://crates.io/crates/criterion) crate. Run them with:

```sh
cargo bench --no-run     # compile only
cargo bench              # execute benchmarks and emit reports
```

HTML reports are written to `target/criterion/`.

## Current Benchmarks

`benches/performance.rs` generates a synthetic ledger with 10 000 transactions and measures:

- `ledger_save_10k`: JSON serialization via the persistence layer.
- `ledger_load_10k`: JSON deserialization and schema migration.
- `budget_summary_current`: Budget aggregation for the current period.
- `forecast_window_report`: Forecast generation across the configured window.

Each benchmark records the elapsed time for the core computation only (file-system overhead is included for load/save to imitate real workloads).

## Extending the Suite

- Add additional functions to the `criterion_group!` in `benches/performance.rs` for simulations or recurrence expansion.
- Use `criterion::BenchmarkGroup` to compare multiple ledger sizes (e.g., 1k vs 10k vs 50k transactions).
- Capture baseline results and store them in release notes as part of the certification report.

## Performance Targets

Guidance for v1.0 release:

| Scenario | Target |
| --- | --- |
| Load/save 10k transactions | ≤ 2 seconds |
| Budget summary generation | < 200 ms |
| Forecast/simulation overlays | < 1 second for 1k deltas |

These targets should be revisited after each optimisation cycle. Results and methodology should be captured in the release benchmark report.
