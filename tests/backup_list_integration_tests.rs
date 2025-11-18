use std::sync::{Arc, RwLock};

use budget_core::cli::commands::backup::list_backups;
use budget_core::cli::core::{CliMode, ShellContext};
use budget_core::cli::registry::CommandRegistry;
use budget_core::cli::shell_context::SelectionOverride;
use budget_core::config::{Config, ConfigManager};
use budget_core::core::ledger_manager::LedgerManager;
use budget_core::ledger::{BudgetPeriod, Ledger};
use budget_core::storage::json_backend::JsonStorage;
use dialoguer::theme::ColorfulTheme;
use tempfile::TempDir;

fn build_context(temp: &TempDir) -> ShellContext {
    let storage = JsonStorage::new(Some(temp.path().to_path_buf()), Some(3)).unwrap();
    let manager = Arc::new(RwLock::new(LedgerManager::new(Box::new(storage.clone()))));
    let config_manager = Arc::new(RwLock::new(
        ConfigManager::with_base_dir(temp.path().to_path_buf()).unwrap(),
    ));
    let config = Arc::new(RwLock::new(Config::default()));
    ShellContext {
        mode: CliMode::Script,
        registry: CommandRegistry::new(),
        ledger_manager: manager,
        theme: ColorfulTheme::default(),
        storage,
        config_manager,
        config,
        ledger_path: None,
        active_simulation_name: None,
        selection_override: Some(SelectionOverride::default()),
        current_simulation: None,
        last_command: None,
        running: true,
    }
}

fn push_choices(context: &ShellContext, choices: &[Option<usize>]) {
    if let Some(overrides) = &context.selection_override {
        for choice in choices {
            overrides.push(*choice);
        }
    }
}

fn set_loaded_ledger(context: &mut ShellContext, name: &str) {
    let ledger = Ledger::new(name, BudgetPeriod::monthly());
    {
        let mut manager = context.ledger_manager.write().unwrap();
        manager.set_current(ledger, None, Some(name.into()));
        manager.save_as(name).unwrap();
        manager.backup(None).unwrap();
    }
    context.ledger_path = Some(context.storage.ledger_path(name));
}

#[test]
fn delete_action_removes_backup() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, "Demo");

    push_choices(&context, &[Some(0), Some(1), None]);
    list_backups::run_list_backups(&mut context).unwrap();

    let metadata = context
        .storage
        .list_backup_metadata("Demo")
        .expect("metadata");
    assert!(metadata.is_empty());
}

#[test]
fn escape_leaves_backups_untouched() {
    let temp = TempDir::new().unwrap();
    let mut context = build_context(&temp);
    set_loaded_ledger(&mut context, "Demo");

    push_choices(&context, &[None]);
    list_backups::run_list_backups(&mut context).unwrap();

    let metadata = context
        .storage
        .list_backup_metadata("Demo")
        .expect("metadata");
    assert_eq!(metadata.len(), 1);
}
