use std::{
    io::IsTerminal,
    sync::{OnceLock, RwLock},
};

use colored::{Color, Colorize};

use crate::cli::output::current_preferences;

#[derive(Clone)]
pub struct UiStyle {
    pub header_prefix: String,
    pub separator: char,
    pub horizontal: char,
    pub padding: usize,
    pub use_color: bool,
    pub color_header: Option<Color>,
    pub color_highlight: Option<Color>,
    pub highlight_marker: String,
    pub plain_mode: bool,
    pub use_icons: bool,
}

static STYLE: OnceLock<RwLock<UiStyle>> = OnceLock::new();

pub fn style() -> UiStyle {
    STYLE
        .get_or_init(|| RwLock::new(UiStyle::detect()))
        .read()
        .expect("style lock poisoned")
        .clone()
}

pub fn refresh_style() {
    if let Some(lock) = STYLE.get() {
        if let Ok(mut guard) = lock.write() {
            *guard = UiStyle::detect();
        }
    } else {
        let _ = STYLE.set(RwLock::new(UiStyle::detect()));
    }
}

impl UiStyle {
    fn detect() -> Self {
        let prefs = current_preferences();
        let stdout_tty = std::io::stdout().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let plain_mode = prefs.plain_mode || prefs.screen_reader_mode;
        let use_color = stdout_tty && prefs.color_enabled && !plain_mode && !no_color;
        let use_icons = !plain_mode;

        let header_prefix = if plain_mode {
            "> ".into()
        } else {
            "⮞ ".into()
        };

        Self {
            header_prefix,
            separator: '─',
            horizontal: '─',
            padding: 1,
            use_color,
            color_header: if use_color {
                Some(Color::BrightBlue)
            } else {
                None
            },
            color_highlight: if use_color { Some(Color::Cyan) } else { None },
            highlight_marker: ">".into(),
            plain_mode,
            use_icons,
        }
    }

    pub fn horizontal_line(&self, width: usize) -> String {
        self.horizontal.to_string().repeat(width.max(40))
    }

    pub fn apply_header_style(&self, text: &str) -> String {
        if self.use_color {
            match self.color_header {
                Some(color) => text.color(color).bold().to_string(),
                None => text.bold().to_string(),
            }
        } else {
            text.to_string()
        }
    }

    pub fn apply_highlight_style(&self, text: &str) -> String {
        if self.use_color {
            match self.color_highlight {
                Some(color) => text.color(color).bold().to_string(),
                None => text.bold().to_string(),
            }
        } else {
            text.to_string()
        }
    }
}

pub fn format_header(title: &str) -> String {
    let style = style();
    let prefixed = format!("{}{}", style.header_prefix, title);
    style.apply_header_style(&prefixed)
}
