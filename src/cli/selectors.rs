//! Generic selection data contracts used by the CLI auto-selection framework.
//!
//! Phase 13 wires these definitions into a reusable selection manager. The
//! traits and structures declared here focus on the *shape* of selectable
//! items so command handlers and future providers can share a common
//! vocabulary and behave consistently.

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

    /// Runs the selection workflow (render items, collect input) and returns
    /// either the chosen identifier or [`SelectionOutcome::Cancelled`] when the
    /// user aborts.
    fn select(&mut self) -> Result<SelectionOutcome<Self::Id>, Self::Error>;
}
