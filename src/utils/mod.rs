pub mod persistence;

use std::sync::Once;

static TRACING_INIT: Once = Once::new();

/// Initializes the global tracing subscriber with sensible defaults.
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        use tracing_subscriber::{fmt, EnvFilter};

        let filter =
            EnvFilter::from_default_env().add_directive("budget_core=info".parse().unwrap());

        fmt().with_env_filter(filter).init();
    });
}
