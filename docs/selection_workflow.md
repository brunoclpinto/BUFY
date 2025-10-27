# CLI Selection Workflow (Phase 13 Draft)

This note summarizes the selection architecture introduced in Phase 13.

1. **SelectionItem** – common data structure describing each option:
   - `id`: identifier returned to the caller
   - `label`: primary display string
   - `subtitle` and `category` (optional) for context/grouping

2. **SelectionOutcome** – result of a selection attempt: either `Selected(id)`
   or `Cancelled`.

3. **SelectionProvider** – trait implemented per domain (accounts,
   categories, transactions, simulations, backups). Providers must:
   - produce the current `SelectionItem` list via `items()`
   - execute the interaction via `select()`, returning the appropriate
     outcome or a domain specific error

4. **Dispatcher integration** (planned) – command handlers request values from
   the selection manager when required arguments are absent. The dispatcher
   remains agnostic of domain specifics.

This document will evolve as the implementation progresses.
