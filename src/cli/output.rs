use colored::Colorize;
use std::fmt;

/// Message categories used by the CLI output helpers.
#[allow(dead_code)]
pub enum MessageKind {
    Info,
    Success,
    Warning,
    Error,
}

fn format_message(message: impl fmt::Display) -> String {
    message.to_string()
}

pub fn print(kind: MessageKind, message: impl fmt::Display) {
    match kind {
        MessageKind::Info => println!("{}", format_message(message)),
        MessageKind::Success => println!("{}", format_message(message).bright_green()),
        MessageKind::Warning => println!("{}", format_message(message).bright_yellow()),
        MessageKind::Error => println!("{}", format_message(message).bright_red()),
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
