use crate::cli::output::current_preferences;

/// Describes how a column should align its contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

/// Specifies the configuration for a single column in the rendered table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableColumn {
    pub header: String,
    pub min_width: usize,
    pub max_width: Option<usize>,
    pub alignment: Alignment,
}

/// Represents a table with column metadata and rows of data to render.
pub struct Table {
    pub columns: Vec<TableColumn>,
    pub rows: Vec<Vec<String>>,
    pub show_headers: bool,
    pub padding: usize,
}

impl Table {
    /// Computes the content widths for each column based on headers, rows, and
    /// column constraints.
    pub fn compute_widths(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .map(|(idx, column)| {
                let header_width = visible_width(&column.header);
                let mut width = header_width.max(column.min_width);
                for row in &self.rows {
                    if let Some(cell) = row.get(idx) {
                        width = width.max(visible_width(cell));
                    }
                }
                if let Some(max_width) = column.max_width {
                    width = width.min(max_width);
                }
                width
            })
            .collect()
    }

    fn render_header(&self, widths: &[usize]) -> String {
        let header: Vec<String> = self.columns.iter().map(|c| c.header.clone()).collect();
        self.render_row(&header, widths)
    }

    /// Renders a single row using the provided column widths.
    pub fn render_row(&self, row: &[String], widths: &[usize]) -> String {
        let rendered_cells: Vec<String> = self
            .columns
            .iter()
            .enumerate()
            .map(|(idx, column)| {
                let cell_text = row.get(idx).map(|s| s.as_str()).unwrap_or("");
                render_cell(cell_text, widths[idx], &column.alignment, self.padding)
            })
            .collect();

        rendered_cells.join(" ").trim_end().to_string()
    }

    /// Renders the full table, optionally including headers and separators.
    pub fn render(&self) -> String {
        let widths = self.compute_widths();
        let mut out = String::new();

        if self.show_headers {
            out.push_str(&self.render_header(&widths));
            out.push('\n');
            out.push_str(&horizontal_rule(&widths, self.padding));
            if !self.rows.is_empty() {
                out.push('\n');
            }
        }

        for (idx, row) in self.rows.iter().enumerate() {
            out.push_str(&self.render_row(row, &widths));
            if idx < self.rows.len() - 1 {
                out.push('\n');
            }
        }

        out
    }
}

fn visible_width(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut idx = 0;
    let mut width = 0;

    while idx < bytes.len() {
        if bytes[idx] == 0x1b {
            idx += 1;
            if idx < bytes.len() && bytes[idx] == b'[' {
                idx += 1;
                while idx < bytes.len() {
                    let byte = bytes[idx];
                    idx += 1;
                    if (0x40..=0x7E).contains(&byte) {
                        break;
                    }
                }
                continue;
            }
        }

        if let Some(ch) = text[idx..].chars().next() {
            width += 1;
            idx += ch.len_utf8();
        } else {
            break;
        }
    }

    width
}

fn truncate_text(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    if visible_width(text) <= width {
        return text.to_string();
    }

    if width == 1 {
        return "…".to_string();
    }

    let target = width - 1;
    let bytes = text.as_bytes();
    let mut idx = 0;
    let mut visible = 0;
    let mut result = String::new();
    let mut saw_ansi = false;

    while idx < bytes.len() && visible < target {
        if bytes[idx] == 0x1b {
            let start = idx;
            idx += 1;
            if idx < bytes.len() && bytes[idx] == b'[' {
                idx += 1;
                while idx < bytes.len() {
                    let byte = bytes[idx];
                    idx += 1;
                    if (0x40..=0x7E).contains(&byte) {
                        break;
                    }
                }
            }
            result.push_str(&text[start..idx]);
            saw_ansi = true;
            continue;
        }

        if let Some(ch) = text[idx..].chars().next() {
            let len = ch.len_utf8();
            if visible + 1 > target {
                break;
            }
            result.push_str(&text[idx..idx + len]);
            visible += 1;
            idx += len;
        } else {
            break;
        }
    }

    result.push('…');
    if saw_ansi {
        result.push_str("\u{1b}[0m");
    }
    result
}

/// Renders a single cell with padding and alignment applied.
pub fn render_cell(text: &str, width: usize, alignment: &Alignment, padding: usize) -> String {
    let fitted = truncate_text(text, width);
    let fitted_width = visible_width(&fitted);
    let remaining = width.saturating_sub(fitted_width);

    let (left_spaces, right_spaces) = match alignment {
        Alignment::Left => (0, remaining),
        Alignment::Right => (remaining, 0),
        Alignment::Center => (remaining / 2, remaining - (remaining / 2)),
    };

    let mut cell = String::new();
    cell.push_str(&" ".repeat(padding));
    cell.push_str(&" ".repeat(left_spaces));
    cell.push_str(&fitted);
    cell.push_str(&" ".repeat(right_spaces));
    cell.push_str(&" ".repeat(padding));
    cell
}

/// Builds a horizontal rule that spans the width of the table.
pub fn horizontal_rule(widths: &[usize], padding: usize) -> String {
    if widths.is_empty() {
        return String::new();
    }

    let total_width: usize =
        widths.iter().map(|w| w + (padding * 2)).sum::<usize>() + widths.len().saturating_sub(1);
    let prefs = current_preferences();
    let ch = if prefs.plain_mode { '-' } else { '─' };
    ch.to_string().repeat(total_width)
}
