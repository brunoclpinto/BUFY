use std::{env, fs, path::PathBuf};

fn main() {
    if env::var("CARGO_FEATURE_FFI").is_err() {
        return;
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let header_dir = crate_dir.join("target").join("ffi").join("include");
    if let Err(err) = fs::create_dir_all(&header_dir) {
        panic!("failed to create ffi include directory: {}", err);
    }
    let header_path = header_dir.join("budget_core.h");

    let config_path = crate_dir.join("cbindgen.toml");
    let config = cbindgen::Config::from_file(&config_path).unwrap_or_default();
    cbindgen::generate_with_config(&crate_dir, config)
        .expect("unable to generate FFI header")
        .write_to_file(&header_path);

    println!(
        "cargo:warning=Generated FFI header at {}",
        header_path.display()
    );
}
