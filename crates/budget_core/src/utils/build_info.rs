/// Compile-time build metadata produced by `build.rs`.
#[derive(Debug, Clone, Copy)]
pub struct BuildMetadata {
    pub version: &'static str,
    pub git_hash: &'static str,
    pub git_status: &'static str,
    pub timestamp: &'static str,
    pub target: &'static str,
    pub profile: &'static str,
    pub rustc: &'static str,
}

/// CLI semantic version derived from the crate metadata.
pub const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the statically-embedded build metadata.
pub fn current() -> BuildMetadata {
    BuildMetadata {
        version: env!("CARGO_PKG_VERSION"),
        git_hash: option_env!("BUDGET_CORE_BUILD_HASH").unwrap_or("unknown"),
        git_status: option_env!("BUDGET_CORE_BUILD_STATUS").unwrap_or("unknown"),
        timestamp: option_env!("BUDGET_CORE_BUILD_TIMESTAMP").unwrap_or("unknown"),
        target: option_env!("BUDGET_CORE_BUILD_TARGET").unwrap_or("unknown"),
        profile: option_env!("BUDGET_CORE_BUILD_PROFILE").unwrap_or("unknown"),
        rustc: option_env!("BUDGET_CORE_BUILD_RUSTC").unwrap_or("unknown"),
    }
}
