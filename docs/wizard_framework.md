# Wizard Framework Overview (Phase 14)

Phase 14 introduces a reusable wizard system that drives interactive, multi-step
data entry in the CLI. The framework is intentionally generic: entities describe
their fields declaratively, while the shared engine handles prompts, validation,
navigation, and confirmation.

## Core Types

- `FieldDescriptor` – describes a single input:
  - `key` – unique identifier used to persist or map values.
  - `label` – human-readable prompt.
  - `kind` – high-level type (`Text`, `Integer`, `Decimal`, `Date`, `Time`,
    `Boolean`, or `Choice(Vec<String>)`).
  - `required` – whether the value must be provided.
  - `help` – optional contextual guidance.
  - `validator` – additional rule (`Validator::NonEmpty`, `PositiveNumber`,
    `Date`, `Custom`, etc.).
- `FormDescriptor` – ordered collection of `FieldDescriptor`s that describe the
  entire wizard.
- `FormFlow` – trait implemented by each form; supplies a descriptor, default
  values derived from `ShellContext`/domain data, and transforms collected values
  into a concrete output type.
- `FormEngine` – drives a `FormFlow` using a `FormInteraction` implementation
  (the CLI will provide one backed by `dialoguer` and the central output
  module).
- `FormInteraction` – abstraction over user I/O. The engine asks the
  interaction layer to prompt for fields, honour control commands, and confirm
  the final summary.

## Lifecycle

1. **Initialization** – the engine builds a `FormSession` from the descriptor
   plus optional defaults (when editing existing data).
2. **Field Loop**
   - Prompt for the current field, showing the default value in square brackets
     (`Name [Checking]:`).
   - Read the response and interpret control commands:
     - *Enter / empty input* → `PromptResponse::Keep` (accept default).
     - `cancel` → abort immediately.
     - `back` → revisit the previous field (with a warning if already at
       the start).
     - `help` → display the field’s help text (if any) and re-prompt.
   - On value entry, validate:
     - `FieldKind` enforces coarse rules (numeric/date formats).
     - `Validator` applies domain-specific logic (non-empty, positive, custom
       closure).
     - Errors use `output::warning` and re-prompt without advancing.
   - Successful validation stores the canonicalised value and moves forward.
3. **Completion** – after the final field, the engine builds a summary,
   displays it via `output::info`, and asks for confirmation. The user can:
   - `confirm` to finish,
   - `back` to revisit the last field,
   - `cancel` to abandon the entire wizard.
4. **Return** – the engine returns `FormResult::Completed(Output)` or
   `FormResult::Cancelled`. The caller decides whether to persist or discard
   the output.

## Validation & Defaults

- Common validators are provided out of the box (`NonEmpty`, `PositiveNumber`,
  `Date`, `Time`, `OneOf`). They return canonical strings (e.g. numbers parsed
  and re-serialised, dates reformatted as `YYYY-MM-DD`).
- Developers can supply `Validator::Custom(Arc<dyn Fn(&str) -> Result<String,
  String>>)`, allowing complex domain validation while still integrating with
  the shared error messaging.
- Defaults are supplied through `FormFlow::defaults()`, typically by looking up
  values in `ShellContext` or other domain structures. Defaults appear in prompts
  and are applied when the user presses Enter without typing a value.

## Control Commands

The engine recognises the same control input across all forms:

- **Enter / empty input** → keep default/current value.
- **`back`** → return to the previous field.
- **`help`** → show field help text.
- **`cancel`** → terminate immediately (no state mutations).

Once the CLI integrates the interaction layer, these commands will be exposed
consistently to users in both interactive and screen-reader-friendly modes.

## Testing Philosophy

The framework includes unit tests (`src/cli/forms.rs`) that simulate user input
through a `MockInteraction`. Tests cover:

- Successful completion,
- Validation failures prompting re-entry,
- Cancellation,
- Back navigation,
- Help requests.

Because the engine operates on pure data structures, future tests can provide
domain-specific forms and confirm that entity builders translate form values
into strongly typed results without touching persistent state.

## Next Steps (Phase 15+)

Future phases will:

- Implement concrete forms for accounts, categories, and transactions,
  leveraging `FormFlow` to assemble domain objects.
- Provide a `DialoguerInteraction` that connects the engine to real CLI input,
  ensuring all prompts funnel through the central output helpers.
- Extend validators and field kinds as new domain requirements arise (e.g.,
  currency selectors, recurrence templates).

## Account & Category Implementations (Phase 15)

The first concrete wizards build on this framework:

- **Account wizard** – used by `account add`/`edit`. Fields include Name (required, unique), Type (choice among the `AccountKind` variants), Linked Category (optional; “None” default), Opening Balance (optional numeric), and Notes (optional, trimmed to 512 chars). The edit mode pre-populates defaults from the selected account; the validator prevents renaming collisions and resolves category selections into UUIDs.
- **Category wizard** – used by `category add`/`edit`. Fields cover Name, Kind, Parent Category (optional; candidate list excludes the category being edited and its descendants), Custom vs. Predefined flag, and Notes. Predefined categories lock kind/custom while still allowing parent and note updates.
- Both wizards feed into helper methods (`ShellContext::apply_account_form`, `ShellContext::apply_category_form`) that either create new domain objects or mutate existing ones, invoke `Ledger::touch()`, and surface success/cancel messages via the shared output helpers. Selection providers from Phase 13 supply the target entity when an `edit` command omits the ID.

## Transaction Wizard (Phase 16)

- **Transaction wizard** – `transaction add` and `transaction edit` reuse the same descriptor and validators. Alongside account/category lookups the flow validates dates against the ledger’s creation timestamp, ensures amounts stay non-negative, and normalises optional inputs (`none` → `None`). A compact recurrence preset list covers the common cycles; the “Every N days” branch prompts for a positive integer, while editing flows expose a "Keep existing" option if the stored interval falls outside the preset list.
- **Status-aware defaults** – Completed transactions automatically backfill missing actual dates/amounts with the scheduled date and budgeted value, keeping downstream recurrence metadata consistent.
- **Selector interop** – when users omit an index for `transaction edit/remove/show/complete`, the existing selection manager renders a labelled list: `[##] date | from -> to | amount | category | status`. Empty ledgers short-circuit with a warning message instead of dropping into an empty prompt.
- **Extensibility** – the only touch points for new fields are `TransactionWizard::build` (descriptor/validator/defaults) and `ShellContext::populate_transaction_from_form` (the mutation target). The form engine, confirmation summary, and cancellation behaviour continue to work automatically.
