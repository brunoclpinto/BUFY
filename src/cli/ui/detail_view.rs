use std::cmp;

use crate::cli::ui::style::{format_header, style};
use crate::cli::ui::table_renderer::visible_width;

/// A simple key/value pair for display.
pub struct DetailField {
    pub key: String,
    pub value: String,
}

/// A detail view model: title + fields.
pub struct DetailView {
    pub title: String,
    pub fields: Vec<DetailField>,
}

impl DetailView {
    /// Creates a new detail view with the provided title.
    pub fn new<T: Into<String>>(title: T) -> Self {
        Self {
            title: title.into(),
            fields: Vec::new(),
        }
    }

    /// Adds a field to the view, returning self for chaining.
    pub fn with_field<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.add_field(key, value);
        self
    }

    /// Adds a field to the view in-place.
    pub fn add_field<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.fields.push(DetailField {
            key: key.into(),
            value: value.into(),
        });
    }

    /// Render the detail view as a string (without actions/footer).
    pub fn render(&self) -> String {
        let ui = style();
        let mut lines = Vec::new();
        lines.push("{".to_string());
        lines.extend(self.render_fields());
        lines.push("}".to_string());

        let header = format_header(&self.title);
        let content_width = lines
            .iter()
            .map(|line| visible_width(line))
            .max()
            .unwrap_or(0);
        let total_width = cmp::max(visible_width(&header), content_width);
        let rule = ui.horizontal_line(total_width);

        let mut output = String::new();
        output.push_str(&header);
        output.push('\n');
        output.push_str(&rule);
        output.push('\n');
        for (idx, line) in lines.iter().enumerate() {
            output.push_str(line);
            if idx + 1 < lines.len() {
                output.push('\n');
            }
        }
        output.push('\n');
        output.push_str(&rule);
        output
    }

    fn render_fields(&self) -> Vec<String> {
        if self.fields.is_empty() {
            return Vec::new();
        }

        let max_key_len = self
            .fields
            .iter()
            .map(|field| field.key.len())
            .max()
            .unwrap_or(0);

        self.fields
            .iter()
            .enumerate()
            .map(|(idx, field)| {
                let padded_key = format!("{:width$}", field.key, width = max_key_len);
                let suffix = if idx + 1 == self.fields.len() {
                    ""
                } else {
                    ","
                };
                let value = if field.value.trim().is_empty() {
                    "—".to_string()
                } else {
                    field.value.clone()
                };
                format!("  \"{padded_key}\":  {value}{suffix}")
            })
            .collect()
    }
}
