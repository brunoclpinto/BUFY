# CLI Developer Reference

This guide consolidates the information engineers need when extending or
maintaining the Budget Core CLI once Phase 20 is complete. It complements the
higher-level `docs/design_overview.md` background document.

## Architecture Overview

```
+-------------------+     +------------------+
| cli::shell        |-->--| cli::commands    |
|  - REPL loop      |     |  - handlers      |
|  - history        |     |  - usage strings |
+-------------------+     +------------------+
        |                         |
        v                         v
+-------------------+     +------------------+
| cli::shell_context|<-->--| cli::output     |
|  - active ledger  |     |  - formatting    |
|  - config prefs   |     |  - accessibility |
+-------------------+     +------------------+
        |                         |
        v                         v
+-------------------+     +------------------+
| cli::forms        |     | cli::selection  |
|  - wizards        |     |  - providers     |
|  - validation     |     |  - manager       |
+-------------------+     +------------------+
        |                         |
        v                         v
+-------------------+     +------------------+
| utils::persistence |<--> | ledger::*       |
|  - save/load       |     |  - domain model |
|  - backups         |     |  - summaries    |
+-------------------+     +------------------+
```

The shell orchestrates command execution through the registry. Each handler
uses `ShellContext` to access the current ledger and configuration, delegating UI
responsibilities to the output, forms, and selection modules. Persistence is
isolated in `utils::persistence`, ensuring atomic save/restore semantics.

## Command Registry Reference

| Command | Handler | Category | Notes |
| --- | --- | --- | --- |
| `help`, `version`, `exit` | `cmd_help`, `cmd_version`, `cmd_exit` | Meta | Help reflects live registry contents; `version` prints CLI/build/schema metadata. |
| `new-ledger`, `load`, `save`, `load-ledger`, `save-ledger` | Ledger lifecycle | Persistence | Named saves use the managed store; unnamed paths support ad-hoc JSON files. |
| `backup-ledger`, `list-backups`, `restore-ledger` | Backup control | Persistence | Backups are rotated by retention policy and surfaced through selection lists when no ID is supplied. |
| `config` family | `cmd_config` | Configuration | `show`, `base-currency`, `locale`, `negative-style`, `screen-reader`, `high-contrast`, `valuation`, `backup`, `backups`, `restore`. |
| `add`, `list`, `transaction`, `account`, `category` | CRUD | Wizards/selection | Add/edit commands launch the wizard engine; list commands now share the standardized output helpers. |
| `recurring` | `cmd_recurring` | Recurrence | Supports `list`, `edit`, `clear`, `pause`, `resume`, `skip`, `sync`. |
| `summary`, `forecast` | Reporting | Ledger summaries | Accept optional simulation names and custom windows. |
| `simulation` family, `create-simulation`, `enter-simulation`, `leave-simulation`, `apply-simulation`, `discard-simulation` | Simulation lifecycle | Simulations | Selection prompts appear whenever the identifier is omitted. |

Adding a new command requires registering a `CommandDefinition` in
`build_commands()`; the handler receives `&mut ShellContext` and `&[&str]`, enabling
access to shared state plus the output utilities.

## Wizard & Selection APIs

1. Describe fields via `FieldDescriptor` and assemble them in a `FormDescriptor`.
2. Supply validators: built-ins cover numeric/date/choice checks; custom
   validators share the `Arc<ValidatorCallback>` type.
3. Instantiate `FormEngine::new(&wizard).run(&mut interaction)` to execute the
   flow—interactive mode uses `DialoguerInteraction`, while tests pass a
   deterministic `MockInteraction`.
4. Wizards emit `FormResult::{Completed, Cancelled}`. Handlers inspect the
   result, mutate the ledger, and then lean on `output_success` for messaging.

Selections follow the same pattern: implement `SelectionProvider` (see
`cli::selection::providers`), then call `ShellContext::select_with`. Cancels always
return `None`, ensuring callers decide whether to abort or continue.

## Persistence Specification

- **Ledger files**: `~/.budget_core/<name>.json` using schema version
  `CURRENT_SCHEMA_VERSION` (`v4`). The ledger struct persists accounts,
  categories, transactions, simulations, config, and metadata.
- **Ledger backups**: `~/.budget_core/backups/<slug>/<slug>_YYYYMMDD_HHMM[_note].json`
  created before each save; retention is configurable when constructing the
  storage backend (currently `JsonStorage`).
- **Config backups**: `~/.budget_core/config/backups/config_<timestamp>.json`
  with metadata `{ schema_version: CONFIG_BACKUP_SCHEMA_VERSION, created_at,
  note, config { ... } }`.
- **State file**: `~/.budget_core/state.json` remembers the last opened ledger
  so the CLI can auto-load it in interactive mode.

All writes use atomic temp-file swaps. Restore operations (`restore-ledger`,
`config restore`) validate the schema version before mutating active state.

## Error Handling Policy

- Routing: all user-facing messages pass through `cli::output`.
- Severity levels:
  - `SUCCESS`: green/bold where supported; includes `[✓]` prefix.
  - `INFO`: neutral `[i]` prefix for narrative output.
  - `WARNING`: `[!]` prefix; cancellations and validation hints.
  - `ERROR`: `[x]` prefix; always actionable explanations.
  - `PROMPT`: cyan chevron arrow (`⮞`) preceded by context labels.
- Accessibility flags live inside `OutputPreferences` and can be toggled via
  `config` commands. Screen-reader mode removes colour and restructures tables;
  high-contrast mode bolds important lines; optional audio feedback appends
  `[ding]` to warnings/errors.
- Exception boundaries: command handlers bubble `CommandError` back to
  `ShellContext::process_line`, which converts them to friendly messages without
  dropping the REPL.

## Testing Strategy Snapshot

See `docs/testing_strategy.md` for full detail. Highlights:

- **Unit & integration suites**: executed via `cargo test` / `cargo nextest run`.
  Cover ledger math, recurrence, persistence, simulations, CLI script flows,
  accessibility toggles, and failure injection (atomic save interruption).
- **Stress & regression**: `tests/stress_suite.rs` exercises repeated save/load
  and forecast cycles; `tests/cli_script.rs` preserves command-level
  regressions; `tests/phase2_time_and_recurrence.rs` protects scheduling logic.
- **Manual validation**: Phase 20 requires walkthroughs for ledger creation →
  simulation → forecast, config backup/restore, and accessibility toggles in a
  colourless terminal.

Developers introducing new features must add or update tests in the relevant
suite and refresh documentation references when new commands are exposed.
