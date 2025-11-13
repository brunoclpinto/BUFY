use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Attribute, SetAttribute},
    terminal::{self, ClearType},
    ExecutableCommand,
};

use crate::cli::output::{current_preferences, OutputPreferences};
use crate::cli::ui::formatting::Formatter;

#[derive(Clone, Debug)]
pub struct MenuUI {
    pub title: String,
    pub context: Option<String>,
    pub items: Vec<MenuUIItem>,
}

impl MenuUI {
    pub fn new(title: impl Into<String>, items: Vec<MenuUIItem>) -> Self {
        Self {
            title: title.into(),
            context: None,
            items,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct MenuUIItem {
    pub key: String,
    pub label: String,
    pub description: String,
}

impl MenuUIItem {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug)]
pub enum MenuRenderError {
    Interrupted,
    EndOfInput,
    Io(io::Error),
}

impl From<io::Error> for MenuRenderError {
    fn from(err: io::Error) -> Self {
        MenuRenderError::Io(err)
    }
}

pub struct MenuRenderer {
    prefs: OutputPreferences,
}

impl MenuRenderer {
    pub fn new() -> Self {
        Self {
            prefs: current_preferences(),
        }
    }

    pub fn show(&self, menu: &MenuUI) -> Result<Option<String>, MenuRenderError> {
        if menu.items.is_empty() {
            return Ok(None);
        }

        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout.execute(cursor::Hide)?;

        let mut selected_index = 0usize;
        let max_label_len = menu
            .items
            .iter()
            .map(|item| item.label.len())
            .max()
            .unwrap_or(0);

        let result = loop {
            self.render(&mut stdout, menu, selected_index, max_label_len)?;
            let event = event::read()?;
            match event {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                break Err(MenuRenderError::Interrupted)
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                break Err(MenuRenderError::EndOfInput)
                            }
                            _ => continue,
                        }
                    }
                    match key.code {
                        KeyCode::Up => {
                            selected_index = selected_index
                                .checked_sub(1)
                                .unwrap_or(menu.items.len() - 1);
                        }
                        KeyCode::Down => {
                            selected_index = (selected_index + 1) % menu.items.len();
                        }
                        KeyCode::Home => selected_index = 0,
                        KeyCode::End => selected_index = menu.items.len().saturating_sub(1),
                        KeyCode::PageUp => {
                            selected_index = selected_index.saturating_sub(3);
                        }
                        KeyCode::PageDown => {
                            selected_index =
                                std::cmp::min(selected_index + 3, menu.items.len() - 1);
                        }
                        KeyCode::Enter => {
                            let key = menu.items[selected_index].key.clone();
                            break Ok(Some(key));
                        }
                        KeyCode::Esc => break Ok(None),
                        _ => {}
                    }
                }
                Event::Resize(_, _) => continue,
                Event::Mouse(_) => continue,
                Event::FocusGained | Event::FocusLost | Event::Paste(_) => continue,
            }
        };

        let clear_status = self.clear_screen(&mut stdout);
        stdout.execute(cursor::Show).ok();
        terminal::disable_raw_mode().ok();
        clear_status?;

        result
    }

    fn render(
        &self,
        stdout: &mut Stdout,
        menu: &MenuUI,
        selected_index: usize,
        max_label_len: usize,
    ) -> Result<(), io::Error> {
        self.clear_screen(stdout)?;
        let formatter = Formatter::new();
        if let Some(context) = &menu.context {
            writeln!(stdout, "{}", formatter.detail_text(context))?;
            writeln!(stdout)?;
        }
        writeln!(stdout, "{}", formatter.header_text(&menu.title))?;
        writeln!(stdout)?;

        for (index, item) in menu.items.iter().enumerate() {
            let is_selected = index == selected_index;
            let pointer = if is_selected {
                if self.prefs.plain_mode {
                    ">"
                } else {
                    "▸"
                }
            } else {
                " "
            };
            let row =
                formatter.format_two_column_row(&item.label, &item.description, max_label_len);
            if is_selected {
                stdout.execute(SetAttribute(Attribute::Reverse))?;
            } else {
                stdout.execute(SetAttribute(Attribute::Reset))?;
            }
            write!(stdout, " {pointer} {}", row)?;
            stdout.execute(SetAttribute(Attribute::Reset))?;
            writeln!(stdout)?;
        }

        let hint = formatter.navigation_hint();
        writeln!(stdout)?;
        writeln!(stdout, "{}", formatter.detail_text(hint))?;
        stdout.flush()?;
        Ok(())
    }

    fn clear_screen(&self, stdout: &mut Stdout) -> Result<(), io::Error> {
        stdout.execute(terminal::Clear(ClearType::All))?;
        stdout.execute(cursor::MoveTo(0, 0))?;
        Ok(())
    }
}
