use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    terminal::{self, ClearType},
    ExecutableCommand,
};

use crate::cli::ui::navigation::{navigation_loop, NavKey};
use crossterm::event::KeyCode;
use crate::cli::ui::table_renderer::{horizontal_rule, Table};

const DEFAULT_HIGHLIGHT: &str = "> ";
const DEFAULT_NORMAL: &str = "  ";
const FOOTER_HINT: &str = "Use ↑ ↓ to navigate, Enter to select, ESC to return.";

#[derive(Debug, PartialEq, Eq)]
pub enum ListSelectionResult {
    Selected(usize),
    Escaped,
    Empty,
}

pub struct ListSelector<'a> {
    pub table: &'a Table,
    pub highlight_symbol: &'a str,
    pub normal_symbol: &'a str,
}

impl<'a> ListSelector<'a> {
    pub fn new(table: &'a Table) -> Self {
        Self {
            table,
            highlight_symbol: DEFAULT_HIGHLIGHT,
            normal_symbol: DEFAULT_NORMAL,
        }
    }

    pub fn with_symbols(mut self, highlight: &'a str, normal: &'a str) -> Self {
        self.highlight_symbol = highlight;
        self.normal_symbol = normal;
        self
    }

    pub fn run(&self) -> ListSelectionResult {
        if self.table.rows.is_empty() {
            return ListSelectionResult::Empty;
        }

        if terminal::enable_raw_mode().is_err() {
            return ListSelectionResult::Escaped;
        }

        let mut stdout = io::stdout();
        let cursor_hidden = stdout.execute(cursor::Hide).is_ok();
        let len = self.table.rows.len();
        let mut current_index: usize = 0;

        let result = loop {
            let mut render_error = None;
            let key = navigation_loop(|| {
                if let Err(err) = self.draw(&mut stdout, current_index) {
                    render_error = Some(err);
                }
            });
            if render_error.is_some() {
                break ListSelectionResult::Escaped;
            }

            match key {
                NavKey::Up => {
                    current_index = current_index.checked_sub(1).unwrap_or(len - 1);
                }
                NavKey::Down => {
                    current_index = (current_index + 1) % len;
                }
                NavKey::Enter => break ListSelectionResult::Selected(current_index),
                NavKey::Esc => break ListSelectionResult::Escaped,
                _ => {}
            }
        };

        if cursor_hidden {
            stdout.execute(cursor::Show).ok();
        }
        terminal::disable_raw_mode().ok();
        result
    }

    fn draw(&self, stdout: &mut Stdout, index: usize) -> io::Result<()> {
        stdout.execute(cursor::MoveToColumn(0))?;
        stdout.execute(terminal::Clear(ClearType::FromCursorDown))?;
        writeln!(stdout, "{}", self.render_with_highlight(index))?;
        writeln!(stdout, "{FOOTER_HINT}")?;
        stdout.flush()
    }

    fn render_with_highlight(&self, index: usize) -> String {
        let widths = self.table.compute_widths();
        let mut lines = Vec::new();

        if self.table.show_headers {
            let headers: Vec<String> = self
                .table
                .columns
                .iter()
                .map(|c| c.header.clone())
                .collect();
            lines.push(format!(
                "{}{}",
                self.normal_symbol,
                self.table.render_row(&headers, &widths)
            ));
            lines.push(format!(
                "{}{}",
                self.normal_symbol,
                horizontal_rule(&widths, self.table.padding)
            ));
        }

        for (row_idx, row) in self.table.rows.iter().enumerate() {
            let prefix = if row_idx == index {
                self.highlight_symbol
            } else {
                self.normal_symbol
            };
            lines.push(format!("{}{}", prefix, self.table.render_row(row, &widths)));
        }

        lines.join("\n")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn run_simulated(&self, keys: &[KeyCode]) -> ListSelectionResult {
        if self.table.rows.is_empty() {
            return ListSelectionResult::Empty;
        }

        let len = self.table.rows.len();
        let mut current_index: usize = 0;

        for key in keys {
            match key {
                KeyCode::Up => {
                    current_index = current_index.checked_sub(1).unwrap_or(len - 1);
                }
                KeyCode::Down => {
                    current_index = (current_index + 1) % len;
                }
                KeyCode::Enter => return ListSelectionResult::Selected(current_index),
                KeyCode::Esc => return ListSelectionResult::Escaped,
                _ => {}
            }
        }

        ListSelectionResult::Escaped
    }
}
