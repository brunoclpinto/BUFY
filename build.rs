use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");

    let git_hash = git_rev_parse();
    println!("cargo:rustc-env=BUDGET_CORE_BUILD_HASH={git_hash}");

    let git_status = git_dirty_suffix();
    println!("cargo:rustc-env=BUDGET_CORE_BUILD_STATUS={git_status}");

    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    println!("cargo:rustc-env=BUDGET_CORE_BUILD_TIMESTAMP={timestamp}");

    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string());
    println!("cargo:rustc-env=BUDGET_CORE_BUILD_TARGET={target}");

    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_string());
    println!("cargo:rustc-env=BUDGET_CORE_BUILD_PROFILE={profile}");

    let rustc_version = rustc_version();
    println!("cargo:rustc-env=BUDGET_CORE_BUILD_RUSTC={rustc_version}");
}

fn git_rev_parse() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn git_dirty_suffix() -> String {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| if s.trim().is_empty() { "clean" } else { "dirty" })
        .unwrap_or("unknown")
        .to_string()
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
