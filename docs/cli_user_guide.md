# Budget Core CLI — User Guide

Welcome to the Budget Core command-line interface. This guide explains how to
navigate the shell, use interactive features, manage backups, and keep the
experience accessible on any terminal.

## Getting Started

1. **Launch the CLI**
   ```sh
   cargo run --bin budget_core_cli
   ```
   The prompt shows the active ledger context, e.g. `ledger(home) ⮞`. New
   sessions auto-load the last opened ledger stored in `~/.budget_core/state.json`.

2. **Script Mode**
   ```sh
   BUDGET_CORE_CLI_SCRIPT=1 cargo run --bin budget_core_cli -- < commands.txt
   ```
   Script mode disables interactive prompts; every command must include the
   necessary arguments. Ideal for tests and automation.

3. **Help & Exit**
   - `help` lists all commands; `help <command>` displays usage and a short
     description.
   - `version` prints CLI/build/schema metadata.
   - `exit` quits the shell (you will be prompted if unsaved changes exist).

## Everyday Commands

| Task | Command | Notes |
| --- | --- | --- |
| Create a ledger | `new-ledger Household monthly` | Omitting arguments triggers interactive prompts. |
| Save / load | `save-ledger household`, `load-ledger household` | Named ledgers live under `~/.budget_core/<name>.json`. |
| Accounts & categories | `account add`, `category add`, `list accounts`, `list categories` | Add/edit commands launch wizards with validation and confirmation steps. |
| Transactions | `transaction add`, `transaction edit`, `transaction show`, `transaction remove`, `transaction complete` | When an ID is omitted, you are shown a selection list. |
| Recurring schedules | `recurring list`, `recurring edit`, `recurring pause`, `recurring resume`, `recurring skip`, `recurring sync` | `recurring list overdue` filters to overdue items. |
| Forecasting & summaries | `forecast 90 days`, `forecast Budget-Plan`, `summary current`, `summary custom 2025-01-01 2025-03-31` | Forecast accepts a simulation name as the first argument. |
| Simulations | `create-simulation Vacation`, `enter-simulation Vacation`, `simulation add`, `apply-simulation Vacation`, `discard-simulation Vacation` | `enter-simulation` changes the prompt to include `[sim:name]`. |
| Configuration | `config show`, `config base-currency EUR`, `config locale en-GB`, `config screen-reader on`, `config high-contrast on` | Preferences persist with the ledger and influence output formatting. |

## Interactive Wizards & Selections

- **Wizards** (`account add`, `transaction edit`, etc.) show a step indicator
  (`Step 3 of 10`). Press `Enter` to accept the default value in brackets.
- Controls available at every step: `back`, `cancel`, and (where provided)
  `help`.
- Validation errors are shown inline via `ERROR: [x]` messages; you remain on
  the same field until the input passes validation.
- **Selections** appear when a command needs an identifier but none was
  supplied. Lists are numbered with two-space indentation and include
  instructions at the bottom: `Type cancel to abort.` Cancelling returns you to
  the main prompt with `WARNING: [!] Operation cancelled.`

## Backups & Persistence

- **Ledger backups**: created automatically before every save at
  `~/.budget_core/backups/<slug>/<slug>_YYYYMMDD_HHMM[_note].json`.
- **Config backups**: create snapshots with `config backup [--note <text>]` and
  list them via `config backups`.
- **Restore workflows**:
  - `restore-ledger` and `config restore` accept either a reference (index or
    substring) or launch a selection list when no argument is provided.
  - Restores validate schema versions and confirm the target before writing.
- **Atomic saves** ensure interrupted writes never corrupt the active file. If a
  save fails, the CLI reports an error and leaves the previous file untouched.

## Accessibility & Keyboard Navigation

- **Screen reader mode** (`config screen-reader on`) removes ANSI colour, adds
  textual prefixes, and renders rows as sentences for improved narration.
- **High contrast mode** (`config high-contrast on`) disables colour entirely
  and relies on bold text for emphasis.
- **Audio cues** (`config screen-reader on` + `config high-contrast on`
  with `audio_feedback` enabled in configuration) append `[ding]` to warnings
  and errors.
- Selection lists support arrow keys and numeric shortcuts; pressing `Esc` or
  typing `cancel` aborts the operation safely.

## Troubleshooting

| Symptom | Suggested Action |
| --- | --- |
| `ERROR: [x] Ledger not loaded` | Run `new-ledger <name> <period>` or `load-ledger <name>` first. |
| Validation errors during wizard | Follow the inline guidance (e.g. "Enter a numeric value"); you can type `cancel` to abort without changes. |
| `WARNING: [!] Backup not found` when restoring | Check `list-backups`/`config backups` for the correct reference. |
| Restoring fails with schema mismatch | Upgrade the CLI to the latest version; older backups cannot be loaded by newer schema versions without migration. |
| Disk full / permission denied | The CLI reports the failure and leaves your previous file untouched. Free space or adjust permissions, then retry `save`/`config backup`. |
| Unexpected crash | The top-level handler catches panics and prints `ERROR: [x] Unexpected error`. Restart the CLI; no ledger changes are committed unless `save`/`apply` succeeded. |

## Additional Resources

- Developer reference: `docs/cli_developer_reference.md`
- Accessibility details: `docs/localization_and_accessibility.md`
- Wizard design: `docs/wizard_framework.md`
- Selection providers: `docs/selection_providers.md`

With Phase 20 complete, this user guide should remain accurate for the CLI
core. Update it whenever new commands or configuration options are added.
