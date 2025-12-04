use crate::cli::core::{CommandError, CommandResult, ShellContext};
use crate::cli::help;
use crate::cli::registry::CommandEntry;
use crate::cli::ui::{Table, TableColumn, TableRenderer};
use crate::config::CONFIG_BACKUP_SCHEMA_VERSION;
use crate::utils::build_info;
use bufy_domain::CURRENT_SCHEMA_VERSION;

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
    let mut table = Table::new(
        Some(format!("Budget Core {}", meta.version)),
        vec![TableColumn::new("FIELD", 18), TableColumn::new("VALUE", 32)],
    );
    table.add_row(vec![
        "CLI version".to_string(),
        build_info::CLI_VERSION.to_string(),
    ]);
    table.add_row(vec![
        "Schema ver".to_string(),
        format!("v{}", CURRENT_SCHEMA_VERSION),
    ]);
    table.add_row(vec![
        "Config schema".to_string(),
        format!("v{}", CONFIG_BACKUP_SCHEMA_VERSION),
    ]);
    table.add_row(vec![
        "Build hash".to_string(),
        format!("{} ({})", meta.git_hash, meta.git_status),
    ]);
    table.add_row(vec!["Built at".to_string(), meta.timestamp.to_string()]);
    table.add_row(vec!["Target".to_string(), meta.target.to_string()]);
    table.add_row(vec!["Profile".to_string(), meta.profile.to_string()]);
    table.add_row(vec!["Rustc".to_string(), meta.rustc.to_string()]);
    #[cfg(feature = "ffi")]
    table.add_row(vec![
        "FFI version".to_string(),
        crate::ffi::FFI_VERSION.to_string(),
    ]);

    TableRenderer::render(&table);
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
