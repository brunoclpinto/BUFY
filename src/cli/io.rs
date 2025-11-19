use std::{
    fmt::Display,
    io::{self, Write},
    ops::Deref,
    sync::{OnceLock, RwLock, RwLockReadGuard},
};

use dialoguer::{
    theme::{ColorfulTheme, SimpleTheme, Theme},
    Confirm, Select,
};

use crate::{
    cli::core::CliError,
    cli::output::{self, OutputPreferences},
    cli::ui::{
        formatting::Formatter,
        prompts::{text_input, TextPromptResult},
        style::refresh_style,
    },
    config::Config,
};

static THEME: OnceLock<RwLock<Box<dyn Theme + Send + Sync>>> = OnceLock::new();
static LOCALE: OnceLock<RwLock<String>> = OnceLock::new();

fn theme_lock() -> &'static RwLock<Box<dyn Theme + Send + Sync>> {
    THEME.get_or_init(|| RwLock::new(Box::new(ColorfulTheme::default())))
}

fn locale_lock() -> &'static RwLock<String> {
    LOCALE.get_or_init(|| RwLock::new(String::from("en-US")))
}

fn theme_guard() -> RwLockReadGuard<'static, Box<dyn Theme + Send + Sync>> {
    theme_lock().read().expect("io theme lock poisoned")
}

/// Configure IO behavior based on the active config (theme + locale).
pub fn apply_config(config: &Config) {
    let plain = config
        .theme
        .as_ref()
        .is_some_and(|value| value.eq_ignore_ascii_case("plain"));

    {
        let mut guard = theme_lock()
            .write()
            .expect("io theme lock poisoned for write");
        if plain {
            *guard = Box::new(SimpleTheme);
        } else {
            *guard = Box::new(ColorfulTheme::default());
        }
    }

    {
        let mut guard = locale_lock()
            .write()
            .expect("locale lock poisoned for write");
        *guard = config.locale.clone();
    }

    output::set_preferences(OutputPreferences {
        plain_mode: plain,
        screen_reader_mode: plain,
        high_contrast_mode: plain,
        quiet_mode: false,
        audio_feedback: config.audio_feedback,
        color_enabled: config.ui_color_enabled,
    });
    refresh_style();
}

fn guard_to_theme<'a>(
    guard: &'a RwLockReadGuard<'static, Box<dyn Theme + Send + Sync>>,
) -> &'a dyn Theme {
    guard.deref().as_ref()
}

/// Prompt the user for free-form text input with an optional default.
/// Returns `Ok(None)` when the user cancels with ESC/back/cancel controls.
pub fn prompt_text(label: &str, default: Option<&str>) -> Result<Option<String>, CliError> {
    let formatter = Formatter::new();
    formatter.print_detail(format!("{label}:"));
    if let Some(value) = default {
        formatter.print_detail(format!("Default: {value}"));
    }
    formatter.print_detail("Type a value and press Enter. Press ESC to cancel.");

    loop {
        match text_input(label, default) {
            Ok(TextPromptResult::Value(value)) => return Ok(Some(value)),
            Ok(TextPromptResult::Keep) => {
                let fallback = default.unwrap_or_default();
                return Ok(Some(fallback.to_string()));
            }
            Ok(TextPromptResult::Help) => {
                formatter.print_detail("Type a value and press Enter. Press ESC to cancel.");
            }
            Ok(TextPromptResult::Back)
            | Ok(TextPromptResult::Cancel)
            | Ok(TextPromptResult::Escape) => return Ok(None),
            Err(err) => return Err(CliError::Input(err.to_string())),
        }
    }
}

/// Prompt the user to choose a value from the provided options, returning the index.
pub fn prompt_select_index<T>(label: &str, options: &[T]) -> Result<usize, CliError>
where
    T: Display,
{
    if options.is_empty() {
        return Err(CliError::Input("no options available".into()));
    }
    let guard = theme_guard();
    let theme = guard_to_theme(&guard);
    Select::with_theme(theme)
        .with_prompt(label)
        .items(options)
        .default(0)
        .interact()
        .map_err(|err| CliError::Input(err.to_string()))
}

/// Prompt the user to choose a value, cloning the selected entry.
pub fn prompt_select_value<T>(label: &str, options: &[T]) -> Result<T, CliError>
where
    T: Display + Clone,
{
    let index = prompt_select_index(label, options)?;
    Ok(options[index].clone())
}

/// Prompt the user for confirmation (yes/no).
pub fn confirm_action(label: &str) -> Result<bool, CliError> {
    let guard = theme_guard();
    let theme = guard_to_theme(&guard);
    Confirm::with_theme(theme)
        .with_prompt(label)
        .default(false)
        .interact()
        .map_err(|err| CliError::Command(err.to_string()))
}

pub fn print_info(message: impl Display) {
    Formatter::new().print_info(message);
}

pub fn print_warn(message: impl Display) {
    Formatter::new().print_warning(message);
}

pub fn print_warning(message: impl Display) {
    print_warn(message);
}

pub fn print_error(message: impl Display) {
    Formatter::new().print_error(message);
}

pub fn write_line<W: Write>(mut out: W, text: &str) -> io::Result<()> {
    out.write_all(text.as_bytes())?;
    out.write_all(b"\r\n")?;
    out.flush()?;
    Ok(())
}

pub fn println_text(text: &str) -> io::Result<()> {
    let mut out = io::stdout();
    write_line(&mut out, text)
}

pub fn print_success(message: impl Display) {
    Formatter::new().print_success(message);
}

pub fn print_hint(message: impl Display) {
    Formatter::new().print_detail(message);
}

pub fn print_error_with_hint(error: impl Display, hint: impl Display) {
    print_error(error);
    print_hint(hint);
}
