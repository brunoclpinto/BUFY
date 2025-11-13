use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Attribute, SetAttribute},
    terminal::{self, ClearType},
    ExecutableCommand,
};

use super::main_menu::MenuError;

const SUBMENU_HINT: &str = "Use ↑/↓ to navigate · Enter to select · ESC to return";

#[derive(Clone, Copy)]
pub struct SubMenuItem {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

impl SubMenuItem {
    pub const fn new(key: &'static str, label: &'static str, description: &'static str) -> Self {
        Self {
            key,
            label,
            description,
        }
    }
}

pub struct SubMenu {
    title: &'static str,
    items: Vec<SubMenuItem>,
    selected_index: usize,
    max_label_len: usize,
}

impl SubMenu {
    pub fn new(title: &'static str, items: Vec<SubMenuItem>) -> Self {
        let max_label_len = items.iter().map(|item| item.label.len()).max().unwrap_or(0);
        Self {
            title,
            items,
            selected_index: 0,
            max_label_len,
        }
    }

    pub fn show(&mut self) -> Result<Option<&'static str>, MenuError> {
        if self.items.is_empty() {
            return Ok(None);
        }

        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout.execute(cursor::Hide)?;

        let loop_result = loop {
            self.render(&mut stdout)?;
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
                            _ => continue,
                        }
                    }

                    match key.code {
                        KeyCode::Up => self.move_selection(-1),
                        KeyCode::Down => self.move_selection(1),
                        KeyCode::Home => self.selected_index = 0,
                        KeyCode::End => self.selected_index = self.items.len().saturating_sub(1),
                        KeyCode::PageUp => self.page_selection(-3),
                        KeyCode::PageDown => self.page_selection(3),
                        KeyCode::Enter => {
                            let key = self.items[self.selected_index].key;
                            break Ok(Some(key));
                        }
                        KeyCode::Esc => break Ok(None),
                        _ => continue,
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

        loop_result
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.items.len() as isize;
        if len == 0 {
            return;
        }
        let current = self.selected_index as isize;
        let next = (current + delta).rem_euclid(len);
        self.selected_index = next as usize;
    }

    fn page_selection(&mut self, delta: isize) {
        let len = self.items.len() as isize;
        if len == 0 {
            return;
        }
        let mut next = self.selected_index as isize + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.selected_index = next as usize;
    }

    fn render(&self, stdout: &mut Stdout) -> Result<(), io::Error> {
        self.clear_screen(stdout)?;
        writeln!(stdout, "{} menu", self.title)?;
        writeln!(stdout, "{SUBMENU_HINT}")?;
        writeln!(stdout)?;

        for (index, item) in self.items.iter().enumerate() {
            if index == self.selected_index {
                stdout.execute(SetAttribute(Attribute::Reverse))?;
            } else {
                stdout.execute(SetAttribute(Attribute::Reset))?;
            }
            write!(
                stdout,
                "  {:<width$}  {}",
                item.label,
                item.description,
                width = self.max_label_len + 2
            )?;
            stdout.execute(SetAttribute(Attribute::Reset))?;
            writeln!(stdout)?;
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
