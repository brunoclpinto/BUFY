use std::{fs, path::Path};

use crate::{errors::LedgerError, ledger::Ledger};

/// Writes the provided ledger to disk atomically by staging to a temporary file.
pub fn save_ledger_to_file(ledger: &Ledger, path: &Path) -> Result<(), LedgerError> {
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(ledger)?;
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Loads a ledger snapshot from disk, returning structured errors on failure.
pub fn load_ledger_from_file(path: &Path) -> Result<Ledger, LedgerError> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}
