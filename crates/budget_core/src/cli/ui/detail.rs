use crate::cli::{io, ui::style::UiStyle};

/// Renders JSON-like detail views for single entities.
pub struct DetailViewRenderer;

impl DetailViewRenderer {
    pub fn render<T>(title: T, fields: &[DetailField], style: &UiStyle)
    where
        T: AsRef<str>,
    {
        let title = title.as_ref();
        let prefix = if style.use_icons { "🔎 " } else { "" };
        let heading = format!("{prefix}{title}");
        let _ = io::println_text(&style.apply_header_style(&heading));
        if !style.plain_mode {
            let _ = io::println_text(&style.horizontal_line(title.len().max(4)));
        }
        let _ = io::println_text("{");
        for (idx, field) in fields.iter().enumerate() {
            let comma = if idx + 1 == fields.len() { "" } else { "," };
            let line = format!("  \"{}\": {}{}", field.label, field.value, comma);
            let _ = io::println_text(&line);
        }
        let _ = io::println_text("}");
        if !style.plain_mode {
            let _ = io::println_text(&style.horizontal_line(title.len().max(4)));
        }
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
