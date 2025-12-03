use budget_core::{cli::run_cli, init};

fn main() {
    init();

    if let Err(err) = run_cli() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
