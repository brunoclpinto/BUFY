use std::fmt;

use dialoguer::{theme::ColorfulTheme, Confirm, Input};

use crate::cli::core::CommandError;
use crate::cli::output;

/// Print an informational message via the standard CLI output helpers.
pub fn print_info(message: impl fmt::Display) {
    output::info(message);
}

/// Print a warning message via the standard CLI output helpers.
pub fn print_warning(message: impl fmt::Display) {
    output::warning(message);
}

/// Print an error message via the standard CLI output helpers.
pub fn print_error(message: impl fmt::Display) {
    output::error(message);
}

/// Print a success message via the standard CLI output helpers.
pub fn print_success(message: impl fmt::Display) {
    output::success(message);
}

/// Prompt the user for confirmation with a yes/no question.
pub fn confirm_action(
    theme: &ColorfulTheme,
    prompt: &str,
    default: bool,
) -> Result<bool, CommandError> {
    Confirm::with_theme(theme)
        .with_prompt(prompt)
        .default(default)
        .interact()
        .map_err(CommandError::from)
}

/// Prompt the user for free-form text input.
pub fn prompt_text(theme: &ColorfulTheme, prompt: &str) -> Result<String, CommandError> {
    Input::<String>::with_theme(theme)
        .with_prompt(prompt)
        .interact_text()
        .map_err(CommandError::from)
}
