use std::path::PathBuf;

pub use bufy_config::manager::CONFIG_BACKUP_SCHEMA_VERSION;
pub use bufy_config::{AccessibilitySettings, Config, ConfigError, ConfigManager, Theme};

use crate::core::utils::PathResolver;

pub fn default_manager() -> Result<ConfigManager, ConfigError> {
    ConfigManager::with_base_dir(PathResolver::base_dir())
}

pub fn manager_with_base(base: PathBuf) -> Result<ConfigManager, ConfigError> {
    ConfigManager::with_base_dir(base)
}
