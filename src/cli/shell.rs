use std::{
    borrow::Cow,
    fmt,
    io::{self, BufRead},
};

use rustyline::{
    completion::{Completer, Pair},
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    history::DefaultHistory,
    validate::{ValidationContext, ValidationResult, Validator},
    Cmd, Context as ReadlineContext, Editor, Helper, KeyEvent,
};
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
    let mut editor = Editor::<CommandHelper, DefaultHistory>::new()?;
    let helper = CommandHelper::new(context.command_names());
    editor.set_helper(Some(helper));
    editor.bind_sequence(KeyEvent::from('?'), Cmd::Complete);

    loop {
        if !context.running {
            break;
        }
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

struct CommandHelper {
    commands: Vec<String>,
}

impl CommandHelper {
    fn new(names: Vec<&'static str>) -> Self {
        let mut commands: Vec<String> = names
            .into_iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        commands.sort();
        commands.dedup();
        Self { commands }
    }
}

impl Helper for CommandHelper {}

impl Completer for CommandHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &ReadlineContext<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];
        let start = prefix
            .rfind(char::is_whitespace)
            .map(|idx| idx + 1)
            .unwrap_or(0);

        let trimmed = prefix.trim_start();
        if let Some(space_idx) = trimmed.find(char::is_whitespace) {
            let leading = prefix.len().saturating_sub(trimmed.len());
            if pos > leading + space_idx {
                return Ok((start, Vec::new()));
            }
        }

        let needle = prefix[start..].to_ascii_lowercase();
        let candidates = self
            .commands
            .iter()
            .filter(|name| name.starts_with(&needle))
            .map(|name| Pair {
                display: name.clone(),
                replacement: name.clone(),
            })
            .collect();
        Ok((start, candidates))
    }
}

impl Hinter for CommandHelper {
    type Hint = String;
}

impl Highlighter for CommandHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }
}

impl Validator for CommandHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let _ = ctx;
        Ok(ValidationResult::Valid(None))
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
