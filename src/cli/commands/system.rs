use crate::cli::core::{CommandError, CommandResult, ShellContext};
use crate::cli::help;
use crate::cli::io;
use crate::cli::output::section as output_section;
use crate::cli::registry::CommandEntry;
use crate::ledger::ledger::CURRENT_SCHEMA_VERSION;
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
    output_section(format!("Budget Core {}", meta.version));
    io::print_info(format!("  CLI version  : {}", build_info::CLI_VERSION));
    io::print_info(format!("  Schema ver   : v{}", CURRENT_SCHEMA_VERSION));
    io::print_info(format!(
        "  Config schema: v{}",
        CONFIG_BACKUP_SCHEMA_VERSION
    ));
    io::print_info(format!(
        "  Build hash   : {} ({})",
        meta.git_hash, meta.git_status
    ));
    io::print_info(format!("  Built at     : {}", meta.timestamp));
    io::print_info(format!("  Target       : {}", meta.target));
    io::print_info(format!("  Profile      : {}", meta.profile));
    io::print_info(format!("  Rustc        : {}", meta.rustc));
    #[cfg(feature = "ffi")]
    {
        io::print_info(format!("  FFI version  : {}", crate::ffi::FFI_VERSION));
    }
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
