use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{self, ClearType},
    ExecutableCommand,
};

use crate::cli::ui::menu_renderer::{MenuRenderer, MenuUI, MenuUIItem};
use crate::cli::ui::test_mode::{self, TextTestInput};

const BACK_KEY: &str = "__BACK";

pub enum TextPromptResult {
    Value(String),
    Keep,
    Back,
    Help,
    Cancel,
}

pub enum ChoicePromptResult {
    Value(String),
    Back,
    Cancel,
}

pub enum ConfirmationPromptResult {
    Confirm,
    Back,
    Cancel,
}

pub fn text_input(label: &str, default: Option<&str>) -> io::Result<TextPromptResult> {
    if let Some(scripted) = test_mode::next_text_input(label) {
        return Ok(match scripted {
            TextTestInput::Value(value) => TextPromptResult::Value(value),
            TextTestInput::Keep => TextPromptResult::Keep,
            TextTestInput::Back => TextPromptResult::Back,
            TextTestInput::Help => TextPromptResult::Help,
            TextTestInput::Cancel => TextPromptResult::Cancel,
        });
    }

    let mut guard = RawModeGuard::activate()?;
    let mut stdout = io::stdout();
    redraw_input(&mut stdout, "")?;
    let mut buffer = String::new();

    loop {
        let event = event::read()?;
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            guard.deactivate();
                            println!();
                            return Ok(TextPromptResult::Cancel);
                        }
                        KeyCode::Char('u') | KeyCode::Char('U') => {
                            buffer.clear();
                            redraw_input(&mut stdout, &buffer)?;
                            continue;
                        }
                        _ => {}
                    }
                }

                match key.code {
                    KeyCode::Esc => {
                        guard.deactivate();
                        println!();
                        return Ok(TextPromptResult::Cancel);
                    }
                    KeyCode::Enter => {
                        guard.deactivate();
                        println!();
                        return Ok(interpret_buffer(&buffer, default));
                    }
                    KeyCode::Backspace => {
                        buffer.pop();
                        redraw_input(&mut stdout, &buffer)?;
                    }
                    KeyCode::Char(ch) => {
                        buffer.push(ch);
                        redraw_input(&mut stdout, &buffer)?;
                    }
                    KeyCode::Delete => {
                        buffer.clear();
                        redraw_input(&mut stdout, &buffer)?;
                    }
                    _ => {}
                }
            }
            _ => continue,
        }
    }
}

pub fn choice_menu(
    title: &str,
    context_lines: &[String],
    options: &[String],
    default: Option<&str>,
    enable_back: bool,
) -> io::Result<ChoicePromptResult> {
    if options.is_empty() {
        return Ok(ChoicePromptResult::Cancel);
    }

    let mut items: Vec<MenuUIItem> = options
        .iter()
        .map(|label| MenuUIItem::new(label.clone(), label.clone(), ""))
        .collect();
    if enable_back {
        items.push(MenuUIItem::new(
            BACK_KEY,
            "← Back",
            "Return to the previous field",
        ));
    }

    let mut menu = MenuUI::new(title.to_string(), items);
    if let Some(context) = join_context(context_lines) {
        menu = menu.with_context(context);
    }
    if let Some(default_label) = default {
        if let Some(index) = options
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(default_label))
        {
            menu = menu.with_initial_index(index);
        }
    }

    let selection = match MenuRenderer::new().show(&menu) {
        Ok(value) => value,
        Err(_) => return Ok(ChoicePromptResult::Cancel),
    };
    match selection {
        Some(selection) if selection == BACK_KEY => Ok(ChoicePromptResult::Back),
        Some(selection) => Ok(ChoicePromptResult::Value(selection)),
        None => Ok(ChoicePromptResult::Cancel),
    }
}

pub fn confirm_menu(context_lines: &[String]) -> io::Result<ConfirmationPromptResult> {
    let items = vec![
        MenuUIItem::new("confirm", "Confirm", "Apply the collected changes"),
        MenuUIItem::new(
            BACK_KEY,
            "Edit previous field",
            "Return to the wizard and adjust entries",
        ),
        MenuUIItem::new("cancel", "Cancel", "Abort this wizard"),
    ];

    let mut menu = MenuUI::new("Review entries", items);
    if let Some(context) = join_context(context_lines) {
        menu = menu.with_context(context);
    }
    menu = menu.with_initial_index(0);

    let selection = match MenuRenderer::new().show(&menu) {
        Ok(value) => value,
        Err(_) => return Ok(ConfirmationPromptResult::Cancel),
    };
    match selection {
        Some(selection) if selection == "confirm" => Ok(ConfirmationPromptResult::Confirm),
        Some(selection) if selection == BACK_KEY => Ok(ConfirmationPromptResult::Back),
        _ => Ok(ConfirmationPromptResult::Cancel),
    }
}

fn redraw_input(stdout: &mut Stdout, buffer: &str) -> io::Result<()> {
    stdout.execute(cursor::MoveToColumn(0))?;
    stdout.execute(terminal::Clear(ClearType::CurrentLine))?;
    write!(stdout, "> {}", buffer)?;
    stdout.flush()
}

fn interpret_buffer(buffer: &str, default: Option<&str>) -> TextPromptResult {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return if default.is_some() {
            TextPromptResult::Keep
        } else {
            TextPromptResult::Value(String::new())
        };
    }

    match trimmed.to_ascii_lowercase().as_str() {
        ":cancel" | "cancel" => TextPromptResult::Cancel,
        ":back" | "back" => TextPromptResult::Back,
        ":help" | "help" => TextPromptResult::Help,
        ":clear" | "clear" => TextPromptResult::Value(String::new()),
        _ => TextPromptResult::Value(buffer.to_string()),
    }
}

fn join_context(lines: &[String]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let filtered: Vec<String> = lines
        .iter()
        .map(|line| line.trim_end().to_string())
        .collect();
    if filtered.iter().all(|line| line.is_empty()) {
        None
    } else {
        Some(filtered.join("\n"))
    }
}

struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    fn activate() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self { active: true })
    }

    fn deactivate(&mut self) {
        if self.active {
            let _ = terminal::disable_raw_mode();
            self.active = false;
        }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        self.deactivate();
    }
}
