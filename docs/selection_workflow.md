# CLI Selection Workflow (Phase 13)

Phase 13 turns the placeholder selection APIs into a concrete pipeline that
automatically resolves missing identifiers (account/category/transaction ids,
simulation names, backup references) before executing a command. The flow is
deliberately modular so future UI surfaces can reuse it without coupling to the
current CLI implementation.

## Building Blocks

1. **`SelectionItem`** – shared data structure that every provider emits. Each
   entry carries a stable identifier (`id`), a primary `label`, and optional
   `subtitle`/`category` fields that the shell can render for additional
   context.
2. **`SelectionOutcome`** – indicates whether the user picked a value or
   cancelled the prompt. Command handlers can react uniformly regardless of the
   underlying domain.
3. **`SelectionProvider`** – domain-specific adapters responsible for gathering
   items from the current CLI state (`CliState`). Providers exist for accounts,
   categories, transactions, simulations, ledger backups, and configuration
   backups. Each provider returns domain-appropriate ids (e.g. list indices for
   ledger collections, `PathBuf` for backup files) while reusing the shared
   presentation shape. Phase 16 expands the transaction provider to surface
   rich labels of the form `[##] YYYY-MM-DD | From -> To | amount | category | status`,
   making it easier to disambiguate entries when launching the transaction
   wizards or follow-on actions.
4. **`SelectionManager`** – orchestrates user interaction. Providers hand their
   items to the manager, which renders labels and delegates the actual choice to
   a selector function. The default selector uses `dialoguer::Select`, while
   tests can inject deterministic selectors via `ShellContext::set_selection_choices`
   (which feeds a queue consumed by `SelectionManager::choose_with`).

## Runtime Flow

1. **Detection** – command handlers call helper methods such as
   `ShellContext::transaction_index_from_arg` or `ShellContext::resolve_simulation_name`.
   These detect missing arguments and trigger a selection only when interactive
   input (or a queued test override) is available. With Phase 16, the
   `transaction remove/show/complete` commands lean on the same helper, so
   users can simply press Enter to pick from the recent transaction list instead
   of memorising numeric indices.
2. **Enumeration** – the appropriate provider (`AccountSelectionProvider`,
   `TransactionSelectionProvider`, etc.) captures the latest state via
   `SelectionProvider::items()`.
3. **Display** – `SelectionManager` renders labels and uses either the Dialoguer
   selector (`choose_with_dialoguer`) or the queued override to obtain a
   selected index. Items are always printed with two leading spaces and
   pre-numbered (`  1. …`), followed by a `Type cancel or press Esc to abort.`
   hint so scripting and accessibility tooling receive consistent output.
4. **Continuation** – command handlers receive the resulting identifier. If the
   user cancels, the command simply aborts without side effects; otherwise, the
   handler proceeds as if the argument had been supplied explicitly.

The same code path is used across domains, which keeps the dispatcher/registry
agnostic of domain specifics and makes it trivial to add new providers later.

## Testing Hooks

Script mode remains non-interactive, but unit tests can still exercise
selection-driven branches by pushing `Option<usize>` values into the
`SelectionOverride` queue (`ShellContext::set_selection_choices`). This bypasses the
Dialoguer TTY requirements, keeps command behaviour deterministic, and ensures
that cancel flows are covered alongside the happy paths.
