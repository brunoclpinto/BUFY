# Release Management Checklist

Phase 11 requires a formalised release workflow so every platform receives reproducible, versioned artifacts. This document records the checklist we will follow for the upcoming v1.0 launch and future patch releases.

## Versioning Strategy

- **Core crate** – Semantic versioning (`major.minor.patch`) stored in `Cargo.toml`. Increment:
  - `major` when ledger behaviour or JSON schema changes in a backward-incompatible way.
  - `minor` when new features are added without breaking compatibility.
  - `patch` for bug fixes or performance improvements that do not alter public APIs.
- **FFI interface** – Independent `FFI_VERSION` string in `src/ffi/mod.rs`. Bump whenever the ABI or error contracts change. Clients must check both `core_version` and `ffi_version`.
- Record the mapping between crate version, FFI version, and schema version (`CURRENT_SCHEMA_VERSION`) in the release notes.

## Artifact Checklist

1. **Rust crates**
   - `cargo package --allow-dirty` (CI dry-run) then `cargo publish` tagged release.
2. **CLI binary**
   - Build debug artifact for verification (`cargo build`).
   - Produce release binaries for macOS, Linux, and Windows (`cargo build --release --bin budget_core_cli --target <triple>`).
3. **FFI shared libraries**
   - `cargo build --release --features ffi` for each target triple (macOS `.dylib`, Linux `.so`, Windows `.dll`).
   - Copy headers from `target/ffi/include/budget_core.h`.
4. **Documentation bundle**
   - Export Markdown docs to the release package (`README.md`, `docs/**/*.md`).
   - Generate API docs if required (`cargo doc --no-deps --features ffi`).
5. **Benchmark results**
   - Run `cargo bench` and archive `target/criterion/` summary or snapshot the key metrics into the release notes.
6. **Test suite**
   - `cargo test`
- `cargo test --features ffi`
- `cargo test --test stress_suite`
- `cargo nextest run`
- `cargo clippy --all-targets -- -D warnings`
- `cargo run --bin budget_core_cli -- version` to record compiled metadata for the release notes.

## Metadata & Tagging

- Embed build metadata (git commit hash, timestamp) into the CLI and FFI via a `build.rs` helper (planned in a subsequent step).
- Tag the repository `vX.Y.Z` after all artifacts are built and tests pass.
- Publish a changelog entry summarising features, bug fixes, performance notes, and benchmark deltas.
- Update integration guides with the released FFI version and supported targets.

## Distribution Targets

- **GitHub Releases** – Upload CLI binaries, FFI libraries + headers, documentation archive, benchmark summary, and checksums.
- **Crates.io** – Publish the Rust library (`budget_core`).
- **Bindings** – Share Swift Package, Gradle AAR, and NuGet packages once language-specific wrappers ship (Phase 9 follow-up).

This checklist will evolve as CI automation lands; treat it as the authoritative reference until scripting makes the process turnkey.
