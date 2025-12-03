use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::core::errors::BudgetError;

pub struct PathResolver;

impl PathResolver {
    pub fn base_dir() -> PathBuf {
        if let Some(custom) = env::var_os("BUDGET_CORE_HOME") {
            return PathBuf::from(custom);
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".budget_core")
    }

    pub fn resolve_base(root: Option<PathBuf>) -> PathBuf {
        root.unwrap_or_else(Self::base_dir)
    }

    pub fn ledger_dir() -> PathBuf {
        Self::ledger_dir_in(&Self::base_dir())
    }

    pub fn ledger_dir_in(base: &Path) -> PathBuf {
        base.join("ledgers")
    }

    pub fn backup_dir() -> PathBuf {
        Self::backup_dir_in(&Self::base_dir())
    }

    pub fn backup_dir_in(base: &Path) -> PathBuf {
        base.join("backups")
    }

    pub fn config_dir() -> PathBuf {
        Self::config_dir_in(&Self::base_dir())
    }

    pub fn config_dir_in(base: &Path) -> PathBuf {
        base.join("config")
    }

    pub fn config_backup_dir() -> PathBuf {
        Self::config_backup_dir_in(&Self::base_dir())
    }

    pub fn config_backup_dir_in(base: &Path) -> PathBuf {
        Self::config_dir_in(base).join("backups")
    }

    pub fn simulation_dir() -> PathBuf {
        Self::simulation_dir_in(&Self::base_dir())
    }

    pub fn simulation_dir_in(base: &Path) -> PathBuf {
        base.join("simulations")
    }

    pub fn config_file() -> PathBuf {
        Self::config_file_in(&Self::base_dir())
    }

    pub fn config_file_in(base: &Path) -> PathBuf {
        Self::config_dir_in(base).join("config.json")
    }

    pub fn state_file() -> PathBuf {
        Self::state_file_in(&Self::base_dir())
    }

    pub fn state_file_in(base: &Path) -> PathBuf {
        base.join("state.json")
    }
}

pub fn ensure_dir(path: &Path) -> Result<(), BudgetError> {
    fs::create_dir_all(path).map_err(|err| {
        BudgetError::StorageError(format!(
            "failed to create directory `{}`: {}",
            path.display(),
            err
        ))
    })
}
