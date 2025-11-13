use shell_words::split;
use std::{
    fmt,
    io::{self, BufRead},
};

use crate::cli::core::{CliError, CliMode, CommandError, LoopControl, ShellContext};
use crate::cli::menus::{main_menu, MenuError};
use crate::cli::ui::formatting::Formatter;

pub fn run_cli() -> Result<(), CliError> {
    let mode = if std::env::var_os("BUDGET_CORE_CLI_SCRIPT").is_some() {
        CliMode::Script
    } else {
        CliMode::Interactive
    };

    let mut context = ShellContext::new(mode)?;

    match mode {
        CliMode::Interactive => run_interactive(&mut context),
        CliMode::Script => run_script(&mut context),
    }
}

fn run_interactive(context: &mut ShellContext) -> Result<(), CliError> {
    loop {
        if !context.running {
            break;
        }
        match main_menu::show(context) {
            Ok(Some(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match handle_line(context, trimmed) {
                    Ok(LoopControl::Continue) => {}
                    Ok(LoopControl::Exit) => break,
                    Err(err) => context.report_error(err)?,
                }
            }
            Ok(None) => continue,
            Err(MenuError::Interrupted) => {
                if context.confirm_exit()? {
                    break;
                }
            }
            Err(MenuError::EndOfInput) => {
                Formatter::new().print_detail("Exiting shell.");
                break;
            }
            Err(MenuError::Io(err)) => return Err(err.into()),
        }
    }

    Ok(())
}

fn run_script(context: &mut ShellContext) -> Result<(), CliError> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        if !context.running {
            break;
        }
        let line = line?;
        match handle_line(context, &line) {
            Ok(LoopControl::Continue) => {}
            Ok(LoopControl::Exit) => break,
            Err(err) => context.report_error(err)?,
        }
    }
    Ok(())
}

fn handle_line(context: &mut ShellContext, line: &str) -> Result<LoopControl, CommandError> {
    let tokens = match parse_command_line(line) {
        Ok(tokens) => tokens,
        Err(err) => {
            context.print_warning(&err.message);
            return Ok(LoopControl::Continue);
        }
    };

    if tokens.is_empty() {
        return Ok(LoopControl::Continue);
    }

    let raw = &tokens[0];
    let command = raw.to_lowercase();
    let args: Vec<&str> = tokens.iter().skip(1).map(String::as_str).collect();

    context.last_command = Some(line.trim().to_string());

    match context.dispatch(&command, raw, &args) {
        Ok(LoopControl::Exit) => {
            context.running = false;
            Ok(LoopControl::Exit)
        }
        other => other,
    }
}

pub(crate) fn parse_command_line(input: &str) -> Result<Vec<String>, ParseError> {
    split(input).map_err(|err| ParseError {
        message: err.to_string(),
    })
}

#[derive(Debug)]
pub(crate) struct ParseError {
    message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
