use shell_words::split;
use std::{
    fmt,
    io::{self, BufRead},
};

use crate::cli::core::{CliError, CliMode, CommandError, LoopControl, ShellContext};
use crate::cli::menus::main_menu::{MainMenu, MenuError};
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
    let mut menu = MainMenu::new();
    let command_catalog = context.command_names();

    loop {
        if !context.running {
            break;
        }
        let prompt = context.prompt();
        match menu.show(prompt.trim_end(), &command_catalog) {
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
                output_info("Exiting shell.");
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

    let tokens = translate_legacy_command(tokens);

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

pub(crate) fn translate_legacy_command(mut tokens: Vec<String>) -> Vec<String> {
    if tokens.is_empty() {
        return tokens;
    }

    let first_lower = tokens[0].to_ascii_lowercase();
    match first_lower.as_str() {
        "new-ledger" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "new".into());
        }
        "load" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "load".into());
        }
        "load-ledger" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "load-ledger".into());
        }
        "save" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "save".into());
        }
        "save-ledger" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "save-ledger".into());
        }
        "backup-ledger" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "backup".into());
        }
        "list-backups" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "list-backups".into());
        }
        "restore-ledger" => {
            tokens[0] = "ledger".into();
            tokens.insert(1, "restore".into());
        }
        "complete" => {
            tokens[0] = "transaction".into();
            tokens.insert(1, "complete".into());
        }
        "recurring" => {
            tokens[0] = "transaction".into();
            tokens.insert(1, "recurring".into());
        }
        "list-simulations" => {
            tokens[0] = "simulation".into();
            tokens.insert(1, "list".into());
        }
        "create-simulation" => {
            tokens[0] = "simulation".into();
            tokens.insert(1, "create".into());
        }
        "enter-simulation" => {
            tokens[0] = "simulation".into();
            tokens.insert(1, "enter".into());
        }
        "leave-simulation" => {
            tokens[0] = "simulation".into();
            tokens.insert(1, "leave".into());
        }
        "apply-simulation" => {
            tokens[0] = "simulation".into();
            tokens.insert(1, "apply".into());
        }
        "discard-simulation" => {
            tokens[0] = "simulation".into();
            tokens.insert(1, "discard".into());
        }
        "add" => {
            if tokens.len() > 1 {
                let target = tokens[1].to_ascii_lowercase();
                match target.as_str() {
                    "account" | "accounts" => {
                        tokens.remove(1);
                        tokens[0] = "account".into();
                        tokens.insert(1, "add".into());
                    }
                    "category" | "categories" => {
                        tokens.remove(1);
                        tokens[0] = "category".into();
                        tokens.insert(1, "add".into());
                    }
                    "transaction" | "transactions" => {
                        tokens.remove(1);
                        tokens[0] = "transaction".into();
                        tokens.insert(1, "add".into());
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    tokens
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
