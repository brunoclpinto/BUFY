# Budget Core

Budget Core provides a reusable Rust toolkit for building budgeting workflows, simulations, and command-line experiences. Phase 0 establishes a reproducible development environment, initial domain models, and CI automation that future phases will build upon.

## Getting Started

1. Install the stable toolchain and required components:

   ```sh
   rustup toolchain install stable
   rustup component add clippy rustfmt rust-analyzer
   cargo install cargo-nextest cargo-audit cargo-edit
   ```

2. Bootstrap the workspace:

   ```sh
   cargo fmt --all
   cargo build
   cargo test
   cargo nextest run
   cargo clippy --all-targets -- -D warnings
   cargo audit
   ```

3. Launch the CLI harness:

   ```sh
   cargo run --bin budget_core_cli
   ```

Additional architectural notes are captured in `docs/design_overview.md`.

## Development Conventions

- Crate edition: Rust 2021.
- Tracing is initialized via `budget_core::init()` or `budget_core::utils::init_tracing()`.
- All warnings are denied by default (`.cargo/config.toml`), so fix lint issues before committing.

## License

Licensed under either of

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT license (`LICENSE-MIT`)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project is licensed under the same dual license terms.
