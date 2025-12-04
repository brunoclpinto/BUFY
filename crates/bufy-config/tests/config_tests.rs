use bufy_config::{Config, ConfigManager};
use tempfile::tempdir;

#[test]
fn default_config_has_non_empty_fields() {
    let cfg = Config::default();

    assert!(!cfg.currency.is_empty());
    assert!(!cfg.locale.is_empty());
}

#[test]
fn config_manager_persists_and_loads_config() {
    let dir = tempdir().expect("tempdir");
    let manager = ConfigManager::new(dir.path().join("config.json"), dir.path().join("backups"));

    let mut cfg = Config::default();
    cfg.currency = "USD".to_string();
    cfg.locale = "en_US".to_string();

    manager.save(&cfg).expect("save config");
    let loaded = manager.load().expect("load config");

    assert_eq!(loaded.currency, "USD");
    assert_eq!(loaded.locale, "en_US");
}
