//! bufy-config
//!
//! Persistent user preferences and configuration model.
//! Owns the Config data structure plus disk persistence helpers.

pub mod error;
pub mod manager;
pub mod model;

pub use error::ConfigError;
pub use manager::ConfigManager;
pub use model::Config;
