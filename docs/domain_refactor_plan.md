# Domain Layer Refactor Plan

## 1. Target Structure

```
src/
 └── domain/
     ├── mod.rs
     ├── common.rs          // shared traits, helper types, serde helpers
     ├── account.rs         // Account struct + AccountKind enum
     ├── category.rs        // Category struct + CategoryKind enum
     ├── transaction.rs     // Transaction struct, recurrence enums, helpers
     └── ledger.rs          // Ledger aggregate + budgeting/simulation structs
```

### Module Responsibilities

- **common.rs**
  - Export foundational traits:
    ```rust
    pub trait Identifiable { fn id(&self) -> Uuid; }
    pub trait NamedEntity { fn name(&self) -> &str; }
    pub trait Displayable { fn display_label(&self) -> String; }
    ```
  - Provide shared type aliases/import re-exports (`Uuid`, `NaiveDate`, serde prelude).
  - House lightweight helpers (e.g., generic label formatting) that multiple entities use.

- **account.rs**
  - Define `Account` and `AccountKind`.
  - Implement `Identifiable`, `NamedEntity`, `Displayable`.
  - Keep constructors/free functions pure (no CLI/logging).

- **category.rs**
  - Define `Category`, `CategoryKind`.
  - Provide hierarchy helpers (e.g., `is_custom`, `parent_id` accessors).
  - Implement shared traits (Identifiable, NamedEntity, Displayable).

- **transaction.rs**
  - Define `Transaction`, `TransactionStatus`, recurrence structs/enums.
  - Implement setters (`set_recurrence`, `mark_completed`) without side effects outside the struct.
  - Expose helper functions for recurrence calculations that currently live in `ledger::recurring`.
  - Ensure structs derive `Serialize`, `Deserialize`, `Clone`, `PartialEq`, `Eq` where applicable.

- **ledger.rs**
  - Aggregate ledger state and budgeting structures (`Ledger`, `BudgetSummary`, simulation types).
  - Abstract currency/policy interactions behind small interfaces to avoid pulling in `crate::currency`.
    - Introduce traits in `common.rs` if needed (e.g., `CurrencyFormatter`).
  - Keep persistence-friendly structs (`BudgetTotals`, `Simulation`) colocated.
  - Ensure methods stay pure (business logic only) and signal errors via `bufy_domain::error` (later step).

## 2. Dependency Rules

- `/domain` **must not** depend on `crate::cli`, `utils::persistence`, or file/network I/O.
- `/domain` may depend on foundational crates: `serde`, `uuid`, `chrono`, `thiserror` (for domain errors).
- Higher layers (CLI, persistence, simulation) import through `bufy_domain::*` re-exports.
- Currency formatting & policy logic should be injected or handled via traits to prevent circular deps.

## 3. Migration Strategy

1. **Introduce Module Skeleton**
   - Add `src/domain` directory with empty modules & re-exports.
   - Define shared traits in `common.rs`.
   - Update `lib.rs` to expose `pub mod domain;`.

2. **Move Simple Entities**
   - Relocate `Account`, `Category` modules first (minimal dependencies).
   - Update all `use crate::ledger::Account` imports to `bufy_domain::account::Account`.
   - Confirm `cargo check`.

3. **Transfer Transaction + Recurrence**
   - Migrate transaction data and recurrence helpers.
   - Move `time_interval` utilities into `bufy_domain::common` or a dedicated module.
   - Adjust call sites (ledger, CLI forms) to new paths.

4. **Refactor Ledger Aggregate**
   - Move `Ledger` and related budgeting/simulation structs.
   - Extract currency-dependent logic into injectable traits or helper structs outside `/domain`.
   - Update persistence layer to use `bufy_domain::ledger::Ledger`.

5. **Clean Up & Document**
   - Remove legacy `src/ledger` directory once all references migrate.
   - Update developer docs and READMEs to reflect the new boundary.

## 4. Risk Mitigations

- **Incremental Commits**: Move one module at a time to keep diffs reviewable.
- **Comprehensive Checks**: Run `cargo check` and targeted tests (if available) after each move.
- **Backwards Compatibility**: Maintain existing method signatures while relocating code to avoid ripple effects.
- **Trait Abstractions**: Introduce minimal traits/adapters rather than embedding CLI/persistence logic in domain structs.

## 5. Next Actions

1. Scaffold `src/domain/{mod,common}.rs` with trait definitions and stub re-exports.
2. Move `Account`/`Category` modules following the plan above.
3. Document any new trait implementations or helper functions required for CLI/Persistence integration.
