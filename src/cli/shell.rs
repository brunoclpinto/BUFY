use std::io::{self, BufRead};

use rustyline::{error::ReadlineError, DefaultEditor};
use shell_words::split;

use crate::cli::core::{CliError, CliMode, CommandError, LoopControl, ShellContext};
use crate::cli::output::info as output_info;

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
    let mut editor = DefaultEditor::new()?;

    loop {
        let prompt = context.prompt();
        let line = editor.readline(&prompt);

        match line {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                editor.add_history_entry(trimmed).ok();

                match handle_line(context, trimmed) {
                    Ok(LoopControl::Continue) => {}
                    Ok(LoopControl::Exit) => break,
                    Err(err) => context.report_error(err)?,
                }
            }
            Err(ReadlineError::Interrupted) => {
                if context.confirm_exit()? {
                    break;
                }
            }
            Err(ReadlineError::Eof) => {
                output_info("Exiting shell.");
                break;
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

fn run_script(context: &mut ShellContext) -> Result<(), CliError> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
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

    context.dispatch(&command, raw, &args)
}

pub(crate) fn parse_command_line(input: &str) -> Result<Vec<String>, ParseError> {
    split(input).map_err(|err| ParseError {
        message: err.to_string(),
    })
}

pub(crate) struct ParseError {
    message: String,
}
