use crate::cli::io;

/// Renders JSON-like detail views for single entities.
pub struct DetailViewRenderer;

impl DetailViewRenderer {
    pub fn render<T>(title: T, fields: &[DetailField])
    where
        T: AsRef<str>,
    {
        let title = title.as_ref();
        let underline = "─".repeat(title.len().max(4));
        let _ = io::println_text(title);
        let _ = io::println_text(&underline);
        let _ = io::println_text("{");
        for (idx, field) in fields.iter().enumerate() {
            let comma = if idx + 1 == fields.len() { "" } else { "," };
            let line = format!("  \"{}\": {}{}", field.label, field.value, comma);
            let _ = io::println_text(&line);
        }
        let _ = io::println_text("}");
    }
}

/// Represents a single key/value pair in a detail view.
#[derive(Debug, Clone)]
pub struct DetailField {
    pub label: String,
    pub value: String,
}

impl DetailField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}
