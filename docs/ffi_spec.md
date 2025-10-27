# Phase 9 – FFI API Blueprint

This document captures the API surface and invariants for exposing the Rust budgeting core to Swift, Kotlin, and C# clients via a stable FFI boundary. It is intentionally detailed so that the implementation in later steps can focus on mechanical correctness without re‑deriving design intent.

## Versioning & Identification

| Identifier | Purpose | Format | Notes |
| --- | --- | --- | --- |
| `CORE_VERSION` | Semantic version of the Rust core (ledger logic, persistence schema). | `major.minor.patch` string. | Mirrors `Cargo.toml` and bumps whenever ledger behavior changes. |
| `FFI_VERSION` | Semantic version for the exported ABI / API surface. | `major.minor.patch` string. | Only bumps when function signatures or error contracts change. |
| `build_metadata` | Optional git hash/timestamp. | String. | Helps GUI clients log the embedded build. |

Every binding must be able to query both identifiers via `ffi_core_version()` and `ffi_version()` before invoking other calls. Newer bindings should gracefully handle older cores (and vice versa) by comparing versions.

## Data Model and Ownership

We expose *opaque handles* for mutable state and pass data across the boundary using JSON payloads or POD structs:

| Handle | Backing Type | Lifetime Rules |
| --- | --- | --- |
| `ffi_ledger_handle` | `Arc<Mutex<LedgerState>>` where `LedgerState` wraps the ledger plus session context. | Created via `ffi_ledger_create/load`, released via `ffi_ledger_free`. Thread-safe (Mutex guarded). |
| `ffi_result_handle` | Boxed error/result objects used for async/reporting scenarios. | Returned by APIs that produce large payloads; caller must free via `ffi_result_free`. |
| `ffi_string` | `*mut c_char` allocated with `CString::into_raw`. | Callers free using `ffi_string_free`. |

Data exchange strategy:

- CRUD inputs (accounts, transactions, categories) use JSON strings to avoid large struct surfaces. The format mirrors the persisted ledger schema.
- Summaries, forecasts, simulations, configuration snapshots are also returned as JSON payloads.
- Lightweight structs (FX tolerance, currency codes, locale settings) are exposed via dedicated POD structs when convenient.

## Error Model

All functions return an `ffi_status` integer. Zero denotes success. Non-zero codes map to categories:

| Code | Category | Examples |
| --- | --- | --- |
| `1` | Validation | Unknown account/category, bad arguments, missing fields. |
| `2` | Persistence | IO errors, serialization failures. |
| `3` | Currency | Missing FX rates, unsupported currency codes. |
| `4` | Simulation | Invalid simulation references, non-editable simulation modifications. |
| `5` | Internal | Unexpected panics caught at the boundary. |

On error, callers must retrieve the last error via `ffi_error_message(out_buffer)` which copies the human-readable message into a caller-provided buffer. (Bindings will wrap this in Result/Exception types.)
Helper APIs already implemented:

- `ffi_last_error_category() -> int` — returns the numeric category of the most recent error (or `0` if none).
- `ffi_last_error_message(char* buffer, size_t len) -> int` — writes the human-readable error into the provided buffer, returning the byte count.
- `ffi_string_free(char*)` — releases strings allocated by the core (e.g., JSON snapshots).

## Module Groups & Operations

### Ledger Lifecycle
-
- `ffi_ledger_create(name, budget_period_json, out_handle)` – initialize a new ledger.
- `ffi_ledger_load(path, out_handle)` – load from JSON file using the persistence layer.
- `ffi_ledger_save(handle, path)` – save to path; uses managed store semantics.
- `ffi_ledger_snapshot(handle, out_json)` – retrieve full ledger JSON for inspection.
- `ffi_ledger_free(handle)` – release resources.

### Accounts & Categories
-
- `ffi_account_add(handle, account_json)` – add/update accounts.
- `ffi_account_list(handle, out_json)` – JSON array of accounts.
- `ffi_category_add`, `ffi_category_list` – analogous for categories.

### Transactions & Recurrence
-
- `ffi_transaction_add(handle, transaction_json)` – create or modify a transaction, including optional recurrence block and currency.
- `ffi_transaction_list(handle, out_json)` – supports filtering window arguments.
- `ffi_recurrence_list(handle, out_json)` – returns `RecurrenceSnapshot` array.
- `ffi_recurring_sync(handle, date)` – materialize due instances.
- `ffi_transaction_forecast(handle, options_json, out_json)` – generate forecast using valuation policy and locale formatting preferences.

### Simulations
-
- `ffi_simulation_create(handle, name, notes)`
- `ffi_simulation_add_tx(handle, sim_name, transaction_json)`
- `ffi_simulation_modify_tx(handle, sim_name, patch_json)`
- `ffi_simulation_apply(handle, sim_name)`
- `ffi_simulation_summary(handle, sim_name, window_json, out_json)`

### Reports & Persistence
-
- `ffi_summary_current(handle, out_json)` – budget summary for current period.
- `ffi_summary_custom(handle, window_json, out_json)` – arbitrary window.
- `ffi_persistence_save_named(handle, name)` / `ffi_persistence_load_named(name, out_handle)`.
- `ffi_backup_create(list, restore)` – wrappers around the existing store features.

### Settings (Currency, Locale, FX)
-
- `ffi_settings_get(handle, out_json)` – returns currency, locale, formatting opts, valuation policy, FX tolerance.
- `ffi_settings_update(handle, settings_json)` – apply new settings.
- `ffi_fx_add(handle, rate_json)` / `ffi_fx_remove(handle, from, to, date)` / `ffi_fx_list(handle, out_json)` / `ffi_fx_tolerance(handle, days)`.

## Thread Safety

- Every `ffi_ledger_handle` wraps `Arc<Mutex<_>>`. All mutable operations lock internally.
- Read-only operations clone the underlying data (`Ledger` implements `Clone`) to minimize contention for large summaries.
- Bindings may use the same handle across threads safely; they must still call `ffi_ledger_free` when finished.

## Serialization Formats

- JSON structures reuse the persisted schema (see `docs/design_overview.md`). This keeps Rust CLI, FFI clients, and persistence aligned.
- Currency/locale settings JSON example:

```json
{
  "base_currency": "USD",
  "locale": { "language_tag": "en-US", "decimal_separator": ".", ... },
  "format": { "currency_display": "symbol", "negative_style": "sign", ... },
  "valuation_policy": { "kind": "transaction_date" },
  "fx_tolerance_days": 5
}
```

Bindings can deserialize into native structs as needed.

## Memory Management Helpers

To avoid leaks each allocation has a paired free function:
-
- `ffi_string_free(*mut c_char)`
- `ffi_result_free(*mut ffi_result)`
- `ffi_ledger_free(*mut ffi_ledger_handle)`

## Build & Artifact Generation

Running `cargo build --features ffi` now produces:

| Artifact | Location | Notes |
| --- | --- | --- |
| C header | `target/ffi/include/budget_core.h` | Generated by the build script using `cbindgen`. Contains declarations for all exported FFI functions and opaque handle types. |
| Shared library | `target/{debug,release}/libbudget_core.{so,dylib,dll}` | Produced because the crate exports both `rlib` and `cdylib`. |

Artifacts are staged under `target/ffi/` so they can be packaged or uploaded by CI. Language-specific modules (Swift package, Kotlin/JNI wrapper, C# P/Invoke) will live under `bindings/` in later steps.

## Next Steps

1. Implement the `ffi` Rust module behind the `ffi` Cargo feature, providing the constants, type definitions, and function stubs described here.
2. Add unit tests around the core API surface to validate JSON contracts and error propagation before generating language-specific bindings.
