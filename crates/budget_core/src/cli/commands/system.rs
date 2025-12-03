use crate::cli::core::{CommandError, CommandResult, ShellContext};
use crate::cli::help;
use crate::cli::registry::CommandEntry;
use crate::cli::ui::formatting::Formatter;
use bufy_domain::CURRENT_SCHEMA_VERSION;
use crate::storage::CONFIG_BACKUP_SCHEMA_VERSION;
use crate::utils::build_info;

pub(crate) fn definitions() -> Vec<CommandEntry> {
    vec![
        CommandEntry::new("version", "Show build metadata", "version", cmd_version),
        CommandEntry::new(
            "help",
            "Show available commands",
            "help [command]",
            cmd_help,
        ),
        CommandEntry::new("exit", "Exit the shell", "exit", cmd_exit),
    ]
}

fn cmd_version(_context: &mut ShellContext, _args: &[&str]) -> CommandResult {
    let meta = build_info::current();
    let formatter = Formatter::new();
    formatter.print_header(format!("Budget Core {}", meta.version));
    let rows = vec![
        ("CLI version", build_info::CLI_VERSION.to_string()),
        ("Schema ver", format!("v{}", CURRENT_SCHEMA_VERSION)),
        (
            "Config schema",
            format!("v{}", CONFIG_BACKUP_SCHEMA_VERSION),
        ),
        (
            "Build hash",
            format!("{} ({})", meta.git_hash, meta.git_status),
        ),
        ("Built at", meta.timestamp.to_string()),
        ("Target", meta.target.to_string()),
        ("Profile", meta.profile.to_string()),
        ("Rustc", meta.rustc.to_string()),
        #[cfg(feature = "ffi")]
        ("FFI version", crate::ffi::FFI_VERSION.to_string()),
    ];
    let borrowed: Vec<_> = rows
        .iter()
        .map(|(label, value)| (*label, value.as_str()))
        .collect();
    formatter.print_two_column(&borrowed);
    Ok(())
}

fn cmd_help(context: &mut ShellContext, args: &[&str]) -> CommandResult {
    if let Some(command) = args.first().map(|name| name.to_lowercase()) {
        if let Some(command) = context.command(&command) {
            help::print_command(command);
        } else {
            context.suggest_command(args[0]);
        }
        return Ok(());
    }

    help::print_overview(&context.registry);
    Ok(())
}

fn cmd_exit(_context: &mut ShellContext, _args: &[&str]) -> CommandResult {
    Err(CommandError::ExitRequested)
}
