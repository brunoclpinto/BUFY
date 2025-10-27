# FFI Bindings Directory

Artifacts produced by the FFI build pipeline are staged under `target/ffi/`:

```
$ cargo build --features ffi --release
# -> target/ffi/include/budget_core.h
# -> target/release/libbudget_core.{dylib,so,dll}
```

Language-specific wrappers (Swift, Kotlin, C#) will live in subdirectories here in later Phase 9 steps. Each binding should document:

- How to include the generated shared library and header.
- Error handling conventions (mapping `ffi_last_error_*`).
- Memory management (ownership of strings/results/handles).
- Version checks using `ffi_core_version` / `ffi_version`.

For now this directory only tracks documentation; CI will upload the generated headers and libraries as build artifacts.
