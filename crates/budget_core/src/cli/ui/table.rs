use crate::cli::{io, ui::style::UiStyle};

/// Declarative description of a table column.
#[derive(Debug, Clone)]
pub struct TableColumn {
    pub header: String,
    pub width: usize,
}

impl TableColumn {
    pub fn new(header: impl Into<String>, width: usize) -> Self {
        Self {
            header: header.into(),
            width,
        }
    }
}

/// Row data for a [`Table`].
#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<String>,
}

/// Simple table model used for rendering read-only overviews.
#[derive(Debug, Clone)]
pub struct Table {
    pub title: Option<String>,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRow>,
}

impl Table {
    pub fn new<T: Into<String>>(title: Option<T>, columns: Vec<TableColumn>) -> Self {
        Self {
            title: title.map(|value| value.into()),
            columns,
            rows: Vec::new(),
        }
    }

    pub fn add_row<S: Into<String>>(&mut self, cells: Vec<S>) {
        let row = TableRow {
            cells: cells.into_iter().map(|value| value.into()).collect(),
        };
        self.rows.push(row);
    }
}

/// Renders [`Table`] instances using simple padded columns.
pub struct TableRenderer;

impl TableRenderer {
    pub fn render(table: &Table, style: &UiStyle) {
        if let Some(title) = &table.title {
            let prefix = if style.use_icons { "📋 " } else { "" };
            let header = format!("{prefix}{title}");
            let _ = io::println_text(&style.apply_header_style(&header));
        }

        if !table.columns.is_empty() {
            let total_width = table
                .columns
                .iter()
                .map(|col| col.width + 1)
                .sum::<usize>()
                .max(1);
            if !style.plain_mode {
                let _ = io::println_text(&style.horizontal_line(total_width));
            }

            let header = table
                .columns
                .iter()
                .map(|col| format!("{:width$} ", col.header, width = col.width))
                .collect::<String>();
            let header_line = style.apply_header_style(header.trim_end());
            let _ = io::println_text(&header_line);
            if !style.plain_mode {
                let _ = io::println_text(&style.horizontal_line(total_width));
            }
        }

        for row in &table.rows {
            let mut line = String::new();
            for (idx, column) in table.columns.iter().enumerate() {
                if idx > 0 {
                    line.push(' ');
                }
                let cell = row.cells.get(idx).map(String::as_str).unwrap_or("");
                line.push_str(&format!("{:width$}", cell, width = column.width));
            }
            let _ = io::println_text(&line);
        }
    }
}
