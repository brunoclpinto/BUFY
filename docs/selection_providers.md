# Selection Providers (Phase 13 Checklist)

Providers to implement:

- Accounts: id, name, optional balance summary
- Categories: id, name, parent path
- Transactions: id, label (direction + amount), date
- Simulations: id/name, status, updated_at
- Ledger backups: timestamp, file path
- Config backups: (if distinct) state snapshot entries

Each provider should implement `SelectionProvider` and return
`SelectionItem<String>` (string IDs for now). Label guidance:

- Account: `"{name} — {kind}"`
- Category: `"{name} ({kind})"`
- Transaction: `"{scheduled_date} • {amount} {label}"`
- Simulation: `"{name} [{status}]"`
- Backups: `"{timestamp}"`

Next steps: wire into command flow (Step 4).
