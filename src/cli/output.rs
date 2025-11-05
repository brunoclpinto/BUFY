use colored::Colorize;
use std::fmt;
use std::sync::{OnceLock, RwLock};

/// Message categories used by the CLI output helpers.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Success,
    Warning,
    Error,
    Prompt,
    Section,
    Separator,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputPreferences {
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

fn build_label(kind: MessageKind) -> (&'static str, &'static str) {
    match kind {
        MessageKind::Info => ("INFO", "[i]"),
        MessageKind::Success => ("SUCCESS", "[✓]"),
        MessageKind::Warning => ("WARNING", "[!]"),
        MessageKind::Error => ("ERROR", "[x]"),
        MessageKind::Prompt => ("PROMPT", ">"),
        MessageKind::Section | MessageKind::Separator => ("INFO", ""),
    }
}

fn apply_style(kind: MessageKind, message: impl fmt::Display, prefs: &OutputPreferences) -> String {
    let text = message.to_string();

    let base = match kind {
        MessageKind::Section => format!("=== {} ===", text.trim()),
        MessageKind::Separator => String::from("----------------------------------------"),
        _ => {
            let (label, icon) = build_label(kind);
            if icon.is_empty() {
                format!("{label}: {text}")
            } else {
                format!("{label}: {icon} {text}")
            }
        }
    };

    let mut formatted = base;

    if prefs.audio_feedback && matches!(kind, MessageKind::Warning | MessageKind::Error) {
        formatted.push_str(" [ding]");
    }

    if prefs.screen_reader_mode {
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
        MessageKind::Success => formatted.bright_green().to_string(),
        MessageKind::Warning => formatted.bright_yellow().to_string(),
        MessageKind::Error => formatted.bright_red().to_string(),
        MessageKind::Prompt => formatted.bright_cyan().to_string(),
        MessageKind::Section => formatted.bold().to_string(),
        MessageKind::Separator => formatted.clone(),
        MessageKind::Info => formatted.clone(),
    }
}

pub fn print(kind: MessageKind, message: impl fmt::Display) {
    let prefs = preferences();
    if should_skip(kind, &prefs) {
        return;
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
