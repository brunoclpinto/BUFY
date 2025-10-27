//! Placeholder module for future selection utilities.
//!
//! During Phase 13 this module will host interactive auto-selection
//! helpers. For now we define the expected interface so command
//! handlers can depend on it without implementation details.

/// Describes the contract for presenting a list of items and capturing a choice.
pub trait SelectionProvider {
    type Item;
    type Error;

    /// Presents `items` to the user and returns the selected entry.
    ///
    /// Implementations should block for user input and provide an accessible
    /// display by default. Errors should communicate why the selection failed
    /// (for example, no items available or the user aborted).
    fn select(&mut self, items: &[Self::Item]) -> Result<Self::Item, Self::Error>;
}
