use std::io::{self, Stdout};

use crossterm::{
    cursor,
    event::KeyCode,
    terminal::{self, ClearType},
    ExecutableCommand,
};

use crate::cli::{
    io::write_line,
    ui::navigation::{navigation_loop, NavKey},
};
use crate::cli::ui::style::{format_header, style};

const FOOTER_TEXT: &str = "Press ↑ ↓ to select an action, Enter to execute, ESC to go back.";

/// A single action entry to show under a detail view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailAction {
    pub id: String,
    pub label: String,
    pub description: String,
}

impl DetailAction {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
        }
    }
}

/// Result of the actions menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetailActionResult {
    Selected(DetailAction),
    Escaped,
    Empty,
}

/// Interactive actions menu rendered beneath a detail view.
pub struct DetailActionsMenu {
    pub title: String,
    pub actions: Vec<DetailAction>,
    pub highlight_symbol: String,
    pub normal_symbol: String,
}

impl DetailActionsMenu {
    pub fn new(title: impl Into<String>, actions: Vec<DetailAction>) -> Self {
        let ui = style();
        Self {
            title: title.into(),
            actions,
            highlight_symbol: format!("{} ", ui.highlight_marker),
            normal_symbol: "  ".to_string(),
        }
    }

    pub fn with_symbols(
        mut self,
        highlight_symbol: impl Into<String>,
        normal_symbol: impl Into<String>,
    ) -> Self {
        self.highlight_symbol = highlight_symbol.into();
        self.normal_symbol = normal_symbol.into();
        self
    }

    /// Run the interactive actions menu and return the selection.
    pub fn run(&self) -> DetailActionResult {
        if self.actions.is_empty() {
            return DetailActionResult::Empty;
        }

        if terminal::enable_raw_mode().is_err() {
            return DetailActionResult::Escaped;
        }

        let mut stdout = io::stdout();
        let cursor_hidden = stdout.execute(cursor::Hide).is_ok();
        let mut current_index: usize = 0;
        let len = self.actions.len();

        let result = loop {
            let mut render_error = None;
            let key = navigation_loop(|| {
                if let Err(err) = self.draw(&mut stdout, current_index) {
                    render_error = Some(err);
                }
            });
            if render_error.is_some() {
                break DetailActionResult::Escaped;
            }

            match key {
                NavKey::Up => {
                    current_index = current_index.checked_sub(1).unwrap_or(len - 1);
                }
                NavKey::Down => {
                    current_index = (current_index + 1) % len;
                }
                NavKey::Enter => {
                    break DetailActionResult::Selected(self.actions[current_index].clone())
                }
                NavKey::Esc => break DetailActionResult::Escaped,
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
        let ui = style();
        stdout.execute(cursor::MoveToColumn(0))?;
        stdout.execute(terminal::Clear(ClearType::FromCursorDown))?;
        let rendered = self.render_actions(index);
        write_line(&mut *stdout, &rendered)?;
        let footer_rule = ui.horizontal_line(FOOTER_TEXT.len().max(40));
        write_line(&mut *stdout, &footer_rule)?;
        write_line(&mut *stdout, FOOTER_TEXT)?;
        Ok(())
    }

    fn render_actions(&self, selected_index: usize) -> String {
        let ui = style();
        let max_label_len = self
            .actions
            .iter()
            .map(|action| action.label.len())
            .max()
            .unwrap_or(0);
        let max_desc_len = self
            .actions
            .iter()
            .map(|action| action.description.len())
            .max()
            .unwrap_or(0);
        let rule_len = std::cmp::max(40, max_label_len + max_desc_len + 10);
        let mut lines = Vec::new();
        lines.push(format_header(&self.title));
        lines.push(ui.horizontal_line(rule_len));
        for (idx, action) in self.actions.iter().enumerate() {
            let marker = if idx == selected_index {
                self.highlight_symbol.as_str()
            } else {
                self.normal_symbol.as_str()
            };
            let padded_label = format!("{:width$}", action.label, width = max_label_len);
            let line = format!("  {marker}{padded_label}  {}", action.description);
            let rendered = if idx == selected_index {
                ui.apply_highlight_style(&line)
            } else {
                line
            };
            lines.push(rendered);
        }
        lines.push(ui.horizontal_line(rule_len));
        lines.join("\r\n")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn render_snapshot(&self, selected_index: usize) -> String {
        self.render_actions(selected_index)
    }
}

impl DetailActionsMenu {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn run_simulated(&self, keys: &[KeyCode]) -> DetailActionResult {
        if self.actions.is_empty() {
            return DetailActionResult::Empty;
        }

        let len = self.actions.len();
        let mut current_index: usize = 0;
        for key in keys {
            match key {
                KeyCode::Up => {
                    current_index = current_index.checked_sub(1).unwrap_or(len - 1);
                }
                KeyCode::Down => {
                    current_index = (current_index + 1) % len;
                }
                KeyCode::Enter => {
                    return DetailActionResult::Selected(self.actions[current_index].clone())
                }
                KeyCode::Esc => return DetailActionResult::Escaped,
                _ => {}
            }
        }

        DetailActionResult::Escaped
    }
}
