//! Generic selection data contracts used by the CLI auto-selection framework.
//!
//! The concrete manager/provides live in `cli::selection`, but the core data
//! structures remain in this module so command handlers, providers, and future
//! front ends can share a common vocabulary. Every provider surfaces
//! `SelectionItem`s and command handlers receive uniform `SelectionOutcome`s
//! regardless of the underlying domain.

/// Minimal data required to render a selectable item to the user.
#[derive(Debug, Clone)]
pub struct SelectionItem<ID> {
    /// Stable identifier returned to the caller when the entry is chosen.
    pub id: ID,
    /// Primary label displayed in the list (name, title, etc.).
    pub label: String,
    /// Optional secondary context (balance, date, category, …).
    pub subtitle: Option<String>,
    /// Optional grouping key for categorized displays.
    pub category: Option<String>,
}

impl<ID> SelectionItem<ID> {
    pub fn new(id: ID, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            subtitle: None,
            category: None,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// Outcome of a selection attempt.
pub enum SelectionOutcome<ID> {
    Selected(ID),
    Cancelled,
}

/// Contract implemented by providers that surface selectable items.
pub trait SelectionProvider {
    type Id;
    type Error;

    /// Fetches the current list of selectable items using CLI state.
    fn items(&mut self) -> Result<Vec<SelectionItem<Self::Id>>, Self::Error>;
}
