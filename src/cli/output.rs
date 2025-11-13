use colored::Colorize;
use std::fmt;
use std::sync::{OnceLock, RwLock};

/// Message categories used by the CLI output helpers.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Hint,
    Success,
    Warning,
    Error,
    Prompt,
    Section,
    Separator,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputPreferences {
    pub plain_mode: bool,
    pub screen_reader_mode: bool,
    pub high_contrast_mode: bool,
    pub quiet_mode: bool,
    pub audio_feedback: bool,
}

static PREFERENCES: OnceLock<RwLock<OutputPreferences>> = OnceLock::new();

pub fn set_preferences(prefs: OutputPreferences) {
    let lock = PREFERENCES.get_or_init(|| RwLock::new(OutputPreferences::default()));
    if let Ok(mut guard) = lock.write() {
        *guard = prefs;
    }
}

pub fn current_preferences() -> OutputPreferences {
    preferences()
}

fn preferences() -> OutputPreferences {
    PREFERENCES
        .get_or_init(|| RwLock::new(OutputPreferences::default()))
        .read()
        .map(|guard| *guard)
        .unwrap_or_default()
}

fn should_skip(kind: MessageKind, prefs: &OutputPreferences) -> bool {
    prefs.quiet_mode && matches!(kind, MessageKind::Separator)
}

fn build_label(kind: MessageKind, plain: bool) -> (&'static str, &'static str) {
    if plain {
        return match kind {
            MessageKind::Info => ("INFO", ""),
            MessageKind::Hint => ("HINT", ""),
            MessageKind::Success => ("SUCCESS", ""),
            MessageKind::Warning => ("WARNING", ""),
            MessageKind::Error => ("ERROR", ""),
            MessageKind::Prompt => ("PROMPT", ""),
            MessageKind::Section | MessageKind::Separator => ("INFO", ""),
        };
    }
    match kind {
        MessageKind::Info => ("INFO", "ℹ️"),
        MessageKind::Hint => ("HINT", "💡"),
        MessageKind::Success => ("SUCCESS", "✅"),
        MessageKind::Warning => ("WARNING", "⚠️"),
        MessageKind::Error => ("ERROR", "❌"),
        MessageKind::Prompt => ("PROMPT", "⮞"),
        MessageKind::Section | MessageKind::Separator => ("INFO", ""),
    }
}

fn apply_style(kind: MessageKind, message: impl fmt::Display, prefs: &OutputPreferences) -> String {
    let text = message.to_string();

    let base = match kind {
        MessageKind::Section => format!("=== {} ===", text.trim()),
        MessageKind::Separator => String::from("----------------------------------------"),
        _ => {
            let (label, icon) = build_label(kind, prefs.plain_mode);
            match icon.is_empty() {
                true => format!("{label}: {text}"),
                false => format!("{label}: {icon} {text}"),
            }
        }
    };

    let mut formatted = base;

    if prefs.audio_feedback && matches!(kind, MessageKind::Warning | MessageKind::Error) {
        formatted.push_str(" [ding]");
    }

    if prefs.plain_mode || prefs.screen_reader_mode {
        return formatted;
    }

    if prefs.high_contrast_mode {
        match kind {
            MessageKind::Success
            | MessageKind::Warning
            | MessageKind::Error
            | MessageKind::Section => return formatted.bold().to_string(),
            _ => return formatted,
        }
    }

    match kind {
        MessageKind::Hint | MessageKind::Info | MessageKind::Prompt => formatted.cyan().to_string(),
        MessageKind::Success => formatted.green().to_string(),
        MessageKind::Warning => formatted.yellow().to_string(),
        MessageKind::Error => formatted.red().to_string(),
        MessageKind::Section => formatted.bold().to_string(),
        MessageKind::Separator => formatted.clone(),
    }
}

pub fn print(kind: MessageKind, message: impl fmt::Display) {
    let prefs = preferences();
    if should_skip(kind, &prefs) {
        return;
    }
    if prefs.audio_feedback && matches!(kind, MessageKind::Warning | MessageKind::Error) {
        print!("\x07");
    }
    let formatted = apply_style(kind, message, &prefs);
    match kind {
        MessageKind::Section | MessageKind::Separator => println!("\n{}", formatted),
        _ => println!("{}", formatted),
    }
}

#[allow(dead_code)]
pub fn info(message: impl fmt::Display) {
    print(MessageKind::Info, message);
}

#[allow(dead_code)]
pub fn success(message: impl fmt::Display) {
    print(MessageKind::Success, message);
}

#[allow(dead_code)]
pub fn warning(message: impl fmt::Display) {
    print(MessageKind::Warning, message);
}

pub fn error(message: impl fmt::Display) {
    print(MessageKind::Error, message);
}

#[allow(dead_code)]
pub fn hint(message: impl fmt::Display) {
    print(MessageKind::Hint, message);
}

#[allow(dead_code)]
pub fn prompt(message: impl fmt::Display) {
    print(MessageKind::Prompt, message);
}

#[allow(dead_code)]
pub fn section(title: impl fmt::Display) {
    print(MessageKind::Section, title);
}

#[allow(dead_code)]
pub fn separator() {
    print(MessageKind::Separator, "");
}

#[allow(dead_code)]
pub fn blank_line() {
    if !preferences().quiet_mode {
        println!();
    }
}

struct TableChars {
    top_left: &'static str,
    top_mid: &'static str,
    top_right: &'static str,
    mid_left: &'static str,
    mid_mid: &'static str,
    mid_right: &'static str,
    bottom_left: &'static str,
    bottom_mid: &'static str,
    bottom_right: &'static str,
    horizontal: &'static str,
    vertical: &'static str,
}

fn table_chars(plain: bool) -> TableChars {
    if plain {
        TableChars {
            top_left: "+",
            top_mid: "+",
            top_right: "+",
            mid_left: "+",
            mid_mid: "+",
            mid_right: "+",
            bottom_left: "+",
            bottom_mid: "+",
            bottom_right: "+",
            horizontal: "-",
            vertical: "|",
        }
    } else {
        TableChars {
            top_left: "┌",
            top_mid: "┬",
            top_right: "┐",
            mid_left: "├",
            mid_mid: "┼",
            mid_right: "┤",
            bottom_left: "└",
            bottom_mid: "┴",
            bottom_right: "┘",
            horizontal: "─",
            vertical: "│",
        }
    }
}

fn char_width(value: &str) -> usize {
    value.chars().count()
}

fn draw_line(chars: &TableChars, widths: &[usize], left: &str, mid: &str, right: &str) {
    print!("{}", left);
    for (idx, width) in widths.iter().enumerate() {
        let segment = chars.horizontal.repeat(width + 2);
        print!("{}", segment);
        if idx + 1 == widths.len() {
            println!("{}", right);
        } else {
            print!("{}", mid);
        }
    }
}

fn draw_row(chars: &TableChars, widths: &[usize], cells: &[String]) {
    print!("{}", chars.vertical);
    for (idx, width) in widths.iter().enumerate() {
        let cell = cells.get(idx).map(String::as_str).unwrap_or("");
        let padded = format!(" {:width$} ", cell, width = *width);
        print!("{padded}");
        if idx + 1 == widths.len() {
            println!("{}", chars.vertical);
        } else {
            print!("{}", chars.vertical);
        }
    }
}

/// Renders data as a formatted table, respecting accessibility preferences.
pub fn render_table(headers: &[&str], rows: &[Vec<String>]) {
    if headers.is_empty() {
        return;
    }
    let prefs = preferences();
    let mut widths: Vec<usize> = headers.iter().map(|header| char_width(header)).collect();
    for row in rows {
        if row.len() > widths.len() {
            widths.resize(row.len(), 0);
        }
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(char_width(cell));
        }
    }

    let chars = table_chars(prefs.plain_mode);
    println!();
    draw_line(
        &chars,
        &widths,
        chars.top_left,
        chars.top_mid,
        chars.top_right,
    );
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    draw_row(&chars, &widths, &header_cells);
    draw_line(
        &chars,
        &widths,
        chars.mid_left,
        chars.mid_mid,
        chars.mid_right,
    );
    if rows.is_empty() {
        let empty = vec![String::from("(none)")];
        draw_row(&chars, &widths, &empty);
    } else {
        for row in rows {
            draw_row(&chars, &widths, row);
        }
    }
    draw_line(
        &chars,
        &widths,
        chars.bottom_left,
        chars.bottom_mid,
        chars.bottom_right,
    );
    println!();
}
