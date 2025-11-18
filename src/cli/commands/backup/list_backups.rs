use crate::cli::core::{CliMode, CommandError, CommandResult, ShellContext};
use crate::cli::io as cli_io;
use crate::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crate::cli::ui::detail_view::DetailView;
use crate::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use crate::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crate::cli::ui::test_mode;
use crate::storage::json_backend::BackupMetadata;

pub fn run_list_backups(context: &mut ShellContext) -> CommandResult {
    let ledger_name = match context.require_named_ledger() {
        Ok(name) => name,
        Err(_) => {
            cli_io::print_warning("No named ledger available.");
            return Ok(());
        }
    };

    loop {
        let entries = gather_entries(context, &ledger_name)?;
        if entries.is_empty() {
            cli_io::print_warning("No backups found.");
            return Ok(());
        }

        let table = build_table(&entries);
        match select_row(context, &table, entries.len()) {
            RowSelection::Exit => return Ok(()),
            RowSelection::Index(index) => {
                let entry = &entries[index];
                println!();
                println!("{}", build_detail_view(entry).render());
                handle_actions(context, &ledger_name, entry)?;
                println!();
            }
        }
    }
}

fn gather_entries(
    context: &ShellContext,
    ledger_name: &str,
) -> Result<Vec<BackupMetadata>, CommandError> {
    context
        .storage
        .list_backup_metadata(ledger_name)
        .map_err(CommandError::from_core)
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

fn format_size(size_bytes: u64) -> String {
    let kb = (size_bytes as f64) / 1024.0;
    if kb < 1.0 {
        format!("{} B", size_bytes)
    } else {
        format!("{:.1} KB", kb)
    }
}

enum RowSelection {
    Index(usize),
    Exit,
}

fn select_row(context: &ShellContext, table: &Table, len: usize) -> RowSelection {
    if let Some(choice) = context.take_override_choice() {
        return match choice {
            Some(index) if index < len => RowSelection::Index(index),
            _ => RowSelection::Exit,
        };
    }

    if let Some(keys) = test_mode::next_selector_events("backup_selector") {
        return match ListSelector::new(table).run_simulated(&keys) {
            ListSelectionResult::Selected(index) => RowSelection::Index(index),
            ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
        };
    }

    match ListSelector::new(table).run() {
        ListSelectionResult::Selected(index) => RowSelection::Index(index),
        ListSelectionResult::Escaped | ListSelectionResult::Empty => RowSelection::Exit,
    }
}

fn handle_actions(
    context: &mut ShellContext,
    ledger_name: &str,
    entry: &BackupMetadata,
) -> CommandResult {
    let actions = vec![
        DetailAction::new("restore", "RESTORE", "Restore this backup"),
        DetailAction::new("delete", "DELETE", "Delete this backup"),
    ];

    let action = match choose_action(context, &actions) {
        DetailActionResult::Selected(action) => Some(action),
        DetailActionResult::Escaped | DetailActionResult::Empty => None,
    };

    if let Some(action) = action {
        match action.id.as_str() {
            "restore" => restore_backup(context, ledger_name, entry)?,
            "delete" => delete_backup(context, ledger_name, entry)?,
            _ => {}
        }
    }
    Ok(())
}

fn choose_action(context: &ShellContext, actions: &[DetailAction]) -> DetailActionResult {
    if let Some(choice) = context.take_override_choice() {
        return match choice {
            Some(index) => actions
                .get(index)
                .cloned()
                .map(DetailActionResult::Selected)
                .unwrap_or(DetailActionResult::Escaped),
            None => DetailActionResult::Escaped,
        };
    }

    if let Some(keys) = test_mode::next_action_events("backup_actions") {
        return DetailActionsMenu::new("Actions", actions.to_vec()).run_simulated(&keys);
    }

    DetailActionsMenu::new("Actions", actions.to_vec()).run()
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
        .map_err(CommandError::from_core)
}
