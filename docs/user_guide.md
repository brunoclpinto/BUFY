# User Guide

This guide walks through common Budget Core workflows, accessibility options, and recovery features. See `docs/development.md` if you are extending the project.

## Quick Start

```
ledger new Household monthly
account add
category add
transaction add
ledger save-ledger household
```

Interactive mode prompts for any missing arguments; script mode (`BUDGET_CORE_CLI_SCRIPT=1`) requires you to provide every parameter explicitly.

## Common Workflows

### Create & Save a Ledger

```text
ledger new Household monthly
account add
category add
transaction add
ledger save-ledger household
```

Use `ledger save <path>` for ad-hoc JSON exports or `ledger save-ledger <name>` to store it under `~/Documents/Ledgers/<name>.bfy` (configurable root).

### Add Accounts & Categories

```text
account add
category add
list accounts
list categories
```

Both commands launch multi-step wizards in interactive mode, validating names, types, and optional metadata. Script mode supports the concise forms (`account add <name> <kind>`).

### Edit Transactions

```text
transaction list
transaction edit
transaction complete 5 2025-02-01 120
```

When an index is omitted in interactive mode, the CLI displays a numbered picker and lets you cancel without side effects.

### Run Forecasts & Simulations

```text
forecast 3 months
simulation create paycut
simulation add paycut
forecast paycut custom 2025-03-01 2025-05-31
```

Forecasts and summaries share the same table renderer, so simulated deltas align with baseline values in plain or coloured output.

## Accessibility & Plain Mode

| Setting | Command | Effect |
| --- | --- | --- |
| Plain mode | `config theme plain` | Disables colours/emoji, uses ASCII tables |
| Screen reader | `config screen-reader on` | Replaces grids with sentence-form entries |
| High contrast | `config high-contrast on` | Removes ANSI styling, bolds headings |
| Audio feedback | `config audio-feedback on` | Adds a short beep to warnings/errors |

All settings apply immediately; run `config show` to confirm their status. Combine screen-reader + plain mode when copying transcripts or building narrated demos.

## Backup & Restore

```
ledger backup
ledger list-backups
ledger restore <idx>

config backup "before sync"
config backups
config restore
```

- Ledger backups live under `~/Documents/Ledger/<slug>-backups/` and carry the `.bbfy` extension.
- Configuration backups live under `~/.budget_core/config/backups/`.
- When restoring interactively, the CLI prints the timestamp and note before asking for confirmation.

## Forecast Operations

- `summary` reports the current budget period (or a specified window) with per-category/account totals.
- `forecast [simulation?] [<n> <unit> | custom <start> <end>]` projects inflow/outflow values. Both commands honour `config valuation` (transaction date vs. report date vs. custom date) and locale settings.
- When a simulation name is supplied, the output table includes the net delta column so you can see how the scenario deviates from the baseline ledger.

## Troubleshooting

- `ledger list-backups <name>` followed by `ledger restore` recovers accidentally deleted data.
- `config show` highlights accessibility settings that might be forcing plain output.
- `config theme default` re-enables colour/emoji formatting if required by demos or screenshots.
- Run `cargo run --bin budget_core_cli --features debug_logs` to enable verbose tracing (see `docs/development.md` for more details).
