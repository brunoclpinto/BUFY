use std::cmp;

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
        let mut lines = Vec::new();
        lines.push("{".to_string());
        lines.extend(self.render_fields());
        lines.push("}".to_string());

        let max_line_len = cmp::max(
            self.title.len(),
            lines.iter().map(|line| line.len()).max().unwrap_or(0),
        );
        let rule_len = cmp::max(max_line_len, 40);
        let rule = horizontal_rule(rule_len);

        let mut output = String::new();
        output.push_str(&self.title);
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
                let padding = max_key_len.saturating_sub(field.key.len()) + 2;
                let spacer = " ".repeat(padding);
                let suffix = if idx + 1 == self.fields.len() {
                    ""
                } else {
                    ","
                };
                format!("  \"{}\":{}{}{}", field.key, spacer, field.value, suffix)
            })
            .collect()
    }
}

fn horizontal_rule(len: usize) -> String {
    let width = len.max(1);
    "─".repeat(width)
}
