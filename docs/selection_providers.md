# Selection Providers (Phase 13 Implementation)

Every domain that requires an identifier now exposes a provider implementing
the shared `SelectionProvider` trait. Providers translate CLI state into
`SelectionItem`s so the selection manager can render consistent prompts without
knowing domain details.

## Implemented Providers

| Provider | Identifier | Label / Context | Notes |
| --- | --- | --- | --- |
| `AccountSelectionProvider` | `usize` (ledger index) | Account name with the account kind as subtitle | Indices keep the prompt stable even if accounts are reordered in memory. Handlers convert the index back into the original account id as needed. |
| `CategorySelectionProvider` | `usize` | Category name with kind, optional parent marker in the category field | Parent ids are shown in the `category` field to disambiguate similarly named sub-categories. |
| `TransactionSelectionProvider` | `usize` | Scheduled date (`YYYY-MM-DD`), budgeted/actual amount in subtitle, recurrence hint in label | Labels call out recurring entries (`• recurring`) to make schedule-driven selections easier. |
| `SimulationSelectionProvider` | `String` (simulation name) | Simulation name with status subtitle | Names remain the authoritative handle for simulations, matching CLI commands and persistence. |
| `LedgerBackupSelectionProvider` | `PathBuf` | Backup timestamp (ISO format) with file path in subtitle | Requires an active named ledger; errors bubble through `ProviderError::Store`. |
| `ConfigBackupSelectionProvider` | `PathBuf` | Snapshot label derived from file stem | Scans the `state-backups/` directory for `.json` files. |

All providers share the following conventions:

- **Fresh data** – providers read directly from `CliState` on each invocation so
  transient CLI mutations are immediately reflected in the prompt.
- **Deterministic ordering** – `enumerate()` is used for ledger collections so
  indices are stable and match what list commands display.
- **Clear messaging** – subtitles/categories surface the most relevant context
  (account kind, category type, recurrence status, etc.) while keeping labels
  concise.

Providers return domain-specific errors through `ProviderError`, which the CLI
automatically maps into `CommandError`. Missing ledgers trigger "ledger not
loaded" messages; filesystem failures from `LedgerStore` are passed through so
the user can act on them.
