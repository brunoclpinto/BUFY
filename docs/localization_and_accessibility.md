# Localization & Accessibility Guide

This guide captures the conventions used by Budget Core to present numbers, dates, and currency in a way that is respectful of locale preferences and accessible to assistive technologies. It complements the high-level overview in `docs/design_overview.md`.

## Runtime Settings

| Setting | Command | Purpose |
| --- | --- | --- |
| Base currency | `config base-currency <ISO4217>` | Sets the reporting currency used for summaries and forecasts. Original transaction currencies are preserved. |
| Locale | `config locale <language-tag>` | Adjusts decimal/grouping separators, date formats, and the first weekday. |
| Negative style | `config negative-style <sign|parentheses>` | Controls how negative values are displayed (e.g., `-123.00` vs. `(123.00)`). |
| Screen reader mode | `config screen-reader <on|off>` | Emits explicit wording (“minus 123 US dollars”) instead of symbols, and simplifies table layout for narration. |
| High contrast mode | `config high-contrast <on|off>` | Disables ANSI colour/emoji usage so output remains legible on monochrome displays or terminals with limited palettes. |
| Valuation policy | `config valuation <transaction|report|custom YYYY-MM-DD>` | Controls the disclosure reference date; conversions must be handled manually if currencies differ. |

All settings persist with the ledger JSON so users can customise per-ledger defaults. CLI commands accept overrides (for example, `forecast 90 days --screen-reader`) when temporary changes are required.

## CLI Output Format

- **Status prefixes** – All messages run through `cli::output`, which renders textual prefixes and ascii-friendly icons (`INFO: [i]`, `SUCCESS: [✓]`, `WARNING: [!]`, `ERROR: [x]`, `PROMPT: >`). This keeps transcripts meaningful even when colours are disabled or a screen reader flattens styling.
- **Sections & lists** – Headings use `output::section`, producing `=== Title ===`. Row-style output is indented by two spaces and avoids tabular ASCII art; separators are textual lines (`----------------------------------------`) so narration remains predictable.
- **Selections** – Interactive selectors share a standard layout (`Select an account:`, numbered items, `Type cancel to abort.`). Cancelling always emits `WARNING: [!] Operation cancelled.` for deterministic scripting.
- **Wizards** – Each step displays `Step N of M`, shows defaults in `[square brackets]`, and accepts `back`, `help`, and `cancel`. Validation errors emit `ERROR:` messages inline before re-prompting.
- **Audio cues** – When `audio_feedback` is enabled (future enhancement), warnings and errors append `[ding]` to provide a textual analogue of the terminal bell.

## Formatting Rules

1. **Deterministic rounding** – Currency values are stored internally as `f64` but formatted using the currency’s declared minor units (e.g., JPY→0 decimals, KWD→3). Conversion totals sum raw amounts before rounding to avoid drift.
2. **Grouping & decimal separators** – Derived from `LocaleConfig`. For unknown locales the CLI applies default separators (`.` decimal, `,` grouping) and prints a warning.
3. **Date styles** – The locale determines the short date pattern shown in summaries (`YYYY-MM-DD` vs. `DD/MM/YYYY`). CLI output always includes four-digit years to avoid ambiguity.
4. **Week anchors** – `LocaleConfig.first_weekday` informs weekly budget windows so totals align with the user’s cultural expectations.
5. **Disclosures** – Budget summaries and forecasts include a footer listing the active valuation policy (transaction/report/custom date) so readers understand the reporting context. FX rates are no longer stored or applied automatically.

## Screen Reader Conventions

Screen reader mode modifies output as follows:

- Replaces currency symbols with phrases (e.g., `€ -123.45` becomes “minus 123 euro and 45 cents”).
- Avoids ASCII art tables; instead, rows are rendered as bullet-style lines with field names first (“Category: Housing; Budgeted: …”).
- Emits explicit tokens for status icons (e.g., “status: overdue” instead of a coloured glyph).
- When high-contrast mode is also enabled, the CLI suppresses background colour cues to keep narration consistent with visual output.

Screen reader mode is safe to leave enabled permanently. When disabled, the CLI resumes compact tabular layouts with ANSI colours where supported.

## Internationalisation Workflow

The CLI’s user-facing strings are routed through a catalog (`src/cli/i18n.rs`). Adding a new language involves:

1. Creating a translation map for each message key, including plural forms where necessary.
2. Updating the language selector to recognise the locale (for example, map `pt-PT` to the Portuguese catalogue).
3. Running `cargo test --test cli_script` to confirm prompts render correctly in script mode.

Guidelines:

- Keep placeholders explicit (e.g., `{amount}`, `{date}`) so translators understand context.
- Provide translator comments where wording is domain specific (rent vs. lease, forecast vs. projection).
- For right-to-left locales, avoid inserting direction-sensitive punctuation inside translated strings; rely on the outer formatting layer to add separators.

## Fallback Behaviour & Error Messages

- **Unsupported locale tag** – The CLI logs `Locale 'xx-YY' is not registered; using default separators.` and continues with the default `LocaleConfig`.
- **Unknown currency** – When a transaction currency differs from the ledger base, summaries mark the entry as incomplete. Convert the amount manually or align the account with the base currency.
- **Screen reader disabled in script mode** – Script mode never auto-enables screen reader mode; tests must set it explicitly to keep outputs deterministic.
- **High contrast request on terminals without ANSI support** – If ANSI detection fails, the CLI already emits plain text; enabling high contrast simply suppresses any remaining colour hints.

## Testing Accessibility Paths

- `tests/currency_tests.rs` validates formatting options (decimal/grouping separators, negative styles).
- `tests/cli_script.rs` exercises script-mode commands; extend it with new accessibility scenarios to ensure output remains deterministic.
- Manual verification: `config screen-reader on`, run `summary`, `forecast 30 days`, and `recurring list`. Confirm narration order and wording match expectations.

For broader design context (module overview, schema reference, and CLI architecture) refer to `docs/design_overview.md`. Integration guidance for Swift, Kotlin, and C# bindings is captured in `docs/integration_guides.md`.
