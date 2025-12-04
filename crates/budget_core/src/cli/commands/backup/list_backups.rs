use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::DetailAction;
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::run_selectable_table;
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use bufy_storage_json::BackupMetadata;

pub fn run_list_backups(context: &mut ShellContext) -> CommandResult {
    let ledger_name = match context.require_named_ledger() {
        Ok(name) => name,
        Err(_) => {
            cli_io::print_warning("No named ledger available.");
            return Ok(());
        }
    };

    let gather_name = ledger_name.clone();
    let action_name = ledger_name.clone();
    run_selectable_table(
        context,
        "backup_selector",
        "backup_actions",
        Some("No backups found."),
        move |ctx| gather_entries(ctx, &gather_name),
        build_table,
        build_detail_view,
        |_| build_actions(),
        move |ctx, entry, action| match action.id.as_str() {
            "restore" => restore_backup(ctx, &action_name, entry),
            "delete" => delete_backup(ctx, &action_name, entry),
            _ => Ok(()),
        },
    )
}

fn gather_entries(
    context: &ShellContext,
    ledger_name: &str,
) -> Result<Vec<BackupMetadata>, CommandError> {
    context
        .storage
        .list_backup_metadata(ledger_name)
        .map_err(CommandError::from)
}

fn build_table(entries: &[BackupMetadata]) -> Table {
    let rows = entries
        .iter()
        .map(|entry| {
            vec![
                entry.name.clone(),
                entry
                    .created_at
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_else(|| "—".into()),
                format_size(entry.size_bytes),
            ]
        })
        .collect();

    Table {
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 16,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "CREATED".into(),
                min_width: 19,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "SIZE".into(),
                min_width: 8,
                max_width: None,
                alignment: Alignment::Right,
            },
        ],
        rows,
        show_headers: true,
        padding: 1,
    }
}

fn build_detail_view(entry: &BackupMetadata) -> DetailView {
    DetailView::new(format!("Backup: {}", entry.name))
        .with_field("name", entry.name.clone())
        .with_field(
            "created_at",
            entry
                .created_at
                .map(|ts| ts.to_rfc3339())
                .unwrap_or_else(|| "—".into()),
        )
        .with_field("size_kb", format_size(entry.size_bytes))
}

fn build_actions() -> Vec<DetailAction> {
    vec![
        DetailAction::new("restore", "RESTORE", "Restore this backup"),
        DetailAction::new("delete", "DELETE", "Delete this backup"),
    ]
}

fn format_size(size_bytes: u64) -> String {
    let kb = (size_bytes as f64) / 1024.0;
    if kb < 1.0 {
        format!("{} B", size_bytes)
    } else {
        format!("{:.1} KB", kb)
    }
}

fn restore_backup(
    context: &mut ShellContext,
    ledger_name: &str,
    entry: &BackupMetadata,
) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!(
            "Restore backup \"{}\"? Current ledger state will be replaced.",
            entry.name
        );
        let confirm = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirm {
            cli_io::print_info("Restore cancelled.");
            return Ok(());
        }
    }
    context.restore_backup_from_name(ledger_name, entry.name.clone())
}

fn delete_backup(
    context: &mut ShellContext,
    ledger_name: &str,
    entry: &BackupMetadata,
) -> CommandResult {
    if context.mode == CliMode::Interactive {
        let prompt = format!("Delete backup \"{}\" permanently?", entry.name);
        let confirm = cli_io::confirm_action(&prompt).map_err(CommandError::from)?;
        if !confirm {
            cli_io::print_info("Delete cancelled.");
            return Ok(());
        }
    }
    context
        .storage
        .delete_backup(ledger_name, &entry.name)
        .map_err(CommandError::from)
}
