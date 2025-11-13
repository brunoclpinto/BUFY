use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Attribute, SetAttribute},
    terminal::{self, ClearType},
    ExecutableCommand,
};

const NAV_HINT: &str =
    "Use ↑/↓ to navigate · Enter to run · ESC to stay · Type a command at any time";

#[derive(Clone)]
struct MenuEntry {
    command: &'static str,
    description: &'static str,
}

enum PartialMatch {
    Unique(String),
    Ambiguous,
    None,
}

#[derive(Debug)]
pub enum MenuError {
    Interrupted,
    EndOfInput,
    Io(io::Error),
}

impl From<io::Error> for MenuError {
    fn from(err: io::Error) -> Self {
        MenuError::Io(err)
    }
}

/// Interactive main menu rendered inside the CLI shell loop.
pub struct MainMenu {
    entries: Vec<MenuEntry>,
    selected_index: usize,
    max_command_len: usize,
}

impl MainMenu {
    pub fn new() -> Self {
        let entries = vec![
            MenuEntry {
                command: "ledger",
                description: "Ledger operations",
            },
            MenuEntry {
                command: "account",
                description: "Manage accounts",
            },
            MenuEntry {
                command: "category",
                description: "Manage categories and budgets",
            },
            MenuEntry {
                command: "transaction",
                description: "Manage transactions",
            },
            MenuEntry {
                command: "simulation",
                description: "Manage simulations",
            },
            MenuEntry {
                command: "forecast",
                description: "Forecast upcoming activity",
            },
            MenuEntry {
                command: "summary",
                description: "Show summary",
            },
            MenuEntry {
                command: "list",
                description: "List entities",
            },
            MenuEntry {
                command: "config",
                description: "Preferences",
            },
            MenuEntry {
                command: "help",
                description: "Help information",
            },
            MenuEntry {
                command: "version",
                description: "Version info",
            },
            MenuEntry {
                command: "exit",
                description: "Quit BUFY",
            },
        ];

        let max_command_len = entries
            .iter()
            .map(|entry| entry.command.len())
            .max()
            .unwrap_or(0);

        Self {
            entries,
            selected_index: 0,
            max_command_len,
        }
    }

    /// Render the menu, capture keyboard navigation, and return the selected command.
    ///
    /// `command_catalog` should contain all registered CLI command names to keep typed command
    /// dispatch compatible with the legacy shell, while the menu entries stay focused on the new UX.
    pub fn show(
        &mut self,
        banner: &str,
        command_catalog: &[&'static str],
    ) -> Result<Option<String>, MenuError> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout.execute(cursor::Hide)?;

        let mut pending_message: Option<String> = None;
        let mut buffer = String::new();
        let loop_result = loop {
            self.render(&mut stdout, banner, &buffer, pending_message.as_deref())?;
            pending_message = None;

            let event = event::read()?;
            match event {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                break Err(MenuError::Interrupted)
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                break Err(MenuError::EndOfInput)
                            }
                            KeyCode::Char('l') | KeyCode::Char('L') => {
                                buffer.clear();
                                self.selected_index = 0;
                                continue;
                            }
                            _ => continue,
                        }
                    }

                    match key.code {
                        KeyCode::Up => self.move_selection(-1),
                        KeyCode::Down => self.move_selection(1),
                        KeyCode::Home => self.selected_index = 0,
                        KeyCode::End => self.selected_index = self.entries.len().saturating_sub(1),
                        KeyCode::PageUp => self.page_selection(-3),
                        KeyCode::PageDown => self.page_selection(3),
                        KeyCode::Esc => break Ok(None),
                        KeyCode::Backspace => {
                            buffer.pop();
                            self.align_selection(&buffer);
                        }
                        KeyCode::Delete => {
                            buffer.clear();
                        }
                        KeyCode::Enter => {
                            let trimmed = buffer.trim();
                            if trimmed.is_empty() {
                                let command = self.entries[self.selected_index].command.to_string();
                                break Ok(Some(command));
                            }
                            if trimmed.contains(char::is_whitespace) {
                                break Ok(Some(trimmed.to_string()));
                            }
                            match self.resolve_partial(trimmed, command_catalog) {
                                PartialMatch::Unique(command) => break Ok(Some(command)),
                                PartialMatch::Ambiguous => {
                                    pending_message = Some(format!(
                                        "Ambiguous command prefix `{}`. Keep typing.",
                                        trimmed
                                    ));
                                }
                                PartialMatch::None => break Ok(Some(trimmed.to_string())),
                            }
                        }
                        KeyCode::Char(ch) => {
                            if key.modifiers.contains(KeyModifiers::ALT) {
                                continue;
                            }
                            buffer.push(ch);
                            self.align_selection(&buffer);
                        }
                        KeyCode::Tab => continue,
                        _ => continue,
                    }
                }
                Event::Resize(_, _) => continue,
                Event::Mouse(_) => continue,
                Event::FocusGained | Event::FocusLost | Event::Paste(_) => continue,
            }
        };

        let clear_outcome = self.clear_screen(&mut stdout);
        stdout.execute(cursor::Show).ok();
        terminal::disable_raw_mode().ok();

        clear_outcome?;
        loop_result
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.entries.len() as isize;
        if len == 0 {
            return;
        }
        let current = self.selected_index as isize;
        let next = (current + delta).rem_euclid(len);
        self.selected_index = next as usize;
    }

    fn page_selection(&mut self, delta: isize) {
        let len = self.entries.len() as isize;
        if len == 0 {
            return;
        }
        let current = self.selected_index as isize;
        let mut next = current + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.selected_index = next as usize;
    }

    fn align_selection(&mut self, buffer: &str) {
        let trimmed = buffer.trim();
        if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
            return;
        }
        let needle = trimmed.to_ascii_lowercase();
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.command.to_ascii_lowercase().starts_with(&needle))
        {
            self.selected_index = index;
        }
    }

    fn resolve_partial(&self, input: &str, catalog: &[&'static str]) -> PartialMatch {
        if input.is_empty() {
            return PartialMatch::None;
        }
        let needle = input.to_ascii_lowercase();
        let mut matches: Vec<&str> = Vec::new();

        for entry in &self.entries {
            if entry
                .command
                .to_ascii_lowercase()
                .starts_with(needle.as_str())
            {
                matches.push(entry.command);
            }
        }

        for name in catalog {
            if matches
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(name))
            {
                continue;
            }
            if name.to_ascii_lowercase().starts_with(needle.as_str()) {
                matches.push(name);
            }
        }

        if matches.is_empty() {
            PartialMatch::None
        } else if matches.len() == 1 {
            PartialMatch::Unique(matches[0].to_string())
        } else if let Some(exact) = matches
            .iter()
            .copied()
            .find(|candidate| candidate.eq_ignore_ascii_case(&needle))
        {
            PartialMatch::Unique(exact.to_string())
        } else {
            PartialMatch::Ambiguous
        }
    }

    fn render(
        &self,
        stdout: &mut Stdout,
        banner: &str,
        buffer: &str,
        message: Option<&str>,
    ) -> Result<(), io::Error> {
        self.clear_screen(stdout)?;
        writeln!(stdout, "{banner}")?;
        writeln!(stdout, "{NAV_HINT}")?;
        writeln!(stdout)?;

        for (index, entry) in self.entries.iter().enumerate() {
            if index == self.selected_index {
                stdout.execute(SetAttribute(Attribute::Reverse))?;
            } else {
                stdout.execute(SetAttribute(Attribute::Reset))?;
            }
            write!(
                stdout,
                "  {:<width$}  {}",
                entry.command,
                entry.description,
                width = self.max_command_len + 2
            )?;
            stdout.execute(SetAttribute(Attribute::Reset))?;
            writeln!(stdout)?;
        }

        writeln!(stdout)?;
        writeln!(stdout, "Command ▶ {}", buffer)?;
        if let Some(text) = message {
            writeln!(stdout, "{text}")?;
        }
        stdout.flush()?;
        Ok(())
    }

    fn clear_screen(&self, stdout: &mut Stdout) -> Result<(), io::Error> {
        stdout.execute(terminal::Clear(ClearType::All))?;
        stdout.execute(cursor::MoveTo(0, 0))?;
        Ok(())
    }
}
