use uuid::Uuid;

/// Identifies entities that expose a stable unique identifier.
pub trait Identifiable {
    fn id(&self) -> Uuid;
}

/// Provides access to a human-friendly entity name.
pub trait NamedEntity {
    fn name(&self) -> &str;
}

/// Supplies a presentation-ready label for UI or logs.
pub trait Displayable {
    fn display_label(&self) -> String;
}

// Re-export common dependencies so consumers can rely on this module as a façade.
pub use chrono;
pub use serde;
pub use uuid;
