use std::{
    env,
    io::{self, Read},
    path::PathBuf,
    process,
};

use budget_core::{
    init,
    ledger::{BudgetPeriod, Ledger},
    utils::persistence,
};

fn main() {
    init();

    if let Err(err) = run() {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| {
        print_usage();
        process::exit(1);
    });

    match command.as_str() {
        "new" => {
            let name = args.next().unwrap_or_else(|| {
                print_usage();
                process::exit(1);
            });

            let ledger = Ledger::new(name, BudgetPeriod::default());
            println!("{}", serde_json::to_string_pretty(&ledger)?);
        }
        "save" => {
            let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
                print_usage();
                process::exit(1);
            });
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            let ledger: Ledger = serde_json::from_str(&buffer)?;
            persistence::save_ledger_to_file(&ledger, &path)?;
            println!("Saved ledger to {}", path.display());
        }
        "load" => {
            let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
                print_usage();
                process::exit(1);
            });
            let ledger = persistence::load_ledger_from_file(&path)?;
            println!("{}", serde_json::to_string_pretty(&ledger)?);
        }
        _ => {
            print_usage();
            process::exit(1);
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage: budget_core_cli <command>\n\
         Commands:\n  \
         new <name>\n  \
         save <file.json> < ledger.json\n  \
         load <file.json>"
    );
}
