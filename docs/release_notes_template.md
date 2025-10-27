# Release Notes Template

Use this template when drafting the v1.0 announcement and future releases. Replace bracketed sections with concrete details and link to relevant PRs/issues.

---

## Budget Core vX.Y.Z (YYYY-MM-DD)

**Highlights**
- [Short bullet summarising marquee feature or fix]
- [Second highlight]
- [Optional third highlight]

**Version Matrix**
- Core crate: `X.Y.Z`
- FFI interface: `A.B.C`
- Schema version: `N`
- CLI `version` output: `budget_core_cli version` → copy/paste hash + timestamp.

### 💡 New Features
- [Feature]: [Brief description of functionality and affected commands/APIs]
- [Feature]: …

### 🐞 Fixes & Reliability
- [Fix]: [What was fixed and how to verify]
- Stress & fault coverage: [Link to updated tests or docs/testing_strategy.md section]

### ⚙️ Performance
- Benchmarks: 
  - `ledger_save_10k`: [result] (± [change vs previous release])
  - `ledger_load_10k`: [result]
  - `budget_summary_current`: [result]
  - `forecast_window_report`: [result]
- Notes: [Any performance regressions or optimisations]

### 🌐 Integrations
- Swift/Kotlin/C# bindings: [Supported versions, notable updates, link to integration guides]
- FX rate updates or localization changes: [Details]

### 📦 Artifacts
- CLI binaries:
  - macOS (`budget_core_cli-vX.Y.Z-macos-x86_64.tar.gz`)
  - Linux (`...`)
  - Windows (`...`)
- FFI libraries + headers: [List zipped bundles]
- Documentation bundle: [docs-vX.Y.Z.zip]
- Example ledgers / smoke datasets: [Optional]

### ✅ Validation
- Tests: `cargo test`, `cargo test --features ffi`, `cargo test --test stress_suite`, `cargo nextest run`, `cargo clippy --all-targets -- -D warnings`
- Benchmarks: `cargo bench`
- `budget_core_cli version`: `[output]`

### 📚 Migration Notes
- If upgrading from v?.?.?: [Steps, migrations triggered, warnings to expect]
- Breaking changes: [Explicit callouts]

### 🔗 Resources
- Documentation: <https://…>
- Issue tracker: <https://…>
- Integration guides: `docs/integration_guides.md`

---

Remember to tag the release in git (`git tag vX.Y.Z && git push origin vX.Y.Z`) after the final build passes. Update any downstream bindings to reference the new FFI version.
