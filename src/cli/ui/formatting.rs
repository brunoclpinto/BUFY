use std::fmt;

use colored::Colorize;

use crate::cli::output::{current_preferences, OutputPreferences};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Header,
    Info,
    Detail,
    Success,
    Warning,
    Error,
}

pub struct Formatter {
    prefs: OutputPreferences,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            prefs: current_preferences(),
        }
    }

    pub fn print_header(&self, title: impl fmt::Display) {
        println!("\n{}", self.header_text(title));
    }

    pub fn header_text(&self, title: impl fmt::Display) -> String {
        let text = format!("=== {} ===", title);
        self.apply_style(Style::Header, text)
    }

    pub fn print_info(&self, message: impl fmt::Display) {
        println!("{}", self.apply_style(Style::Info, message));
    }

    pub fn print_detail(&self, message: impl fmt::Display) {
        println!("{}", self.apply_style(Style::Detail, message));
    }

    pub fn detail_text(&self, message: impl fmt::Display) -> String {
        self.apply_style(Style::Detail, message)
    }

    pub fn print_success(&self, message: impl fmt::Display) {
        self.print_line(Style::Success, message);
    }

    pub fn print_warning(&self, message: impl fmt::Display) {
        self.print_line(Style::Warning, message);
    }

    pub fn print_error(&self, message: impl fmt::Display) {
        self.print_line(Style::Error, message);
    }

    fn print_line(&self, style: Style, message: impl fmt::Display) {
        if self.prefs.audio_feedback && matches!(style, Style::Warning | Style::Error) {
            print!("\x07");
        }
        println!("{}", self.apply_style(style, message));
    }

    fn apply_style(&self, style: Style, message: impl fmt::Display) -> String {
        match style {
            Style::Success => self.decorate("✔", "OK:", message, style),
            Style::Warning => self.decorate("⚠", "WARNING:", message, style),
            Style::Error => self.decorate("✖", "ERROR:", message, style),
            Style::Header => {
                let base = format!("=== {} ===", message);
                self.colorize(base, style)
            }
            Style::Info | Style::Detail => message.to_string(),
        }
    }

    fn decorate(
        &self,
        icon: &str,
        plain_label: &str,
        message: impl fmt::Display,
        style: Style,
    ) -> String {
        if self.prefs.plain_mode || self.prefs.screen_reader_mode {
            format!("{plain_label} {}", message)
        } else {
            let base = format!("{icon} {}", message);
            self.colorize(base, style)
        }
    }

    fn colorize(&self, text: String, style: Style) -> String {
        if self.prefs.plain_mode || self.prefs.screen_reader_mode {
            return text;
        }

        if self.prefs.high_contrast_mode {
            return text.bold().to_string();
        }

        match style {
            Style::Success => text.green().to_string(),
            Style::Warning => text.yellow().to_string(),
            Style::Error => text.red().to_string(),
            Style::Header => text.bold().to_string(),
            Style::Info | Style::Detail => text,
        }
    }

    pub fn navigation_hint(&self) -> String {
        "(Use arrow keys to navigate, Enter to select, ESC to go back)".to_string()
    }

    pub fn print_navigation_hint(&self) {
        self.print_detail(self.navigation_hint());
    }

    pub fn print_two_column(&self, entries: &[(&str, &str)]) {
        if entries.is_empty() {
            return;
        }
        let label_width = entries
            .iter()
            .map(|(label, _)| label.len())
            .max()
            .unwrap_or(0);
        for (label, description) in entries {
            println!(
                "  {:<width$}  {}",
                label,
                description,
                width = label_width + 2
            );
        }
    }

    pub fn format_two_column_row(&self, label: &str, description: &str, width: usize) -> String {
        format!("  {:<width$}  {}", label, description, width = width + 2)
    }
}
