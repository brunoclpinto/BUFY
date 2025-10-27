#![allow(dead_code)]
//! Foreign Function Interface (FFI) bootstrap module.
//! Phase 9 exposes the budgeting core to Swift/Kotlin/C# bindings via a
//! stable C ABI. This file currently defines the shared version identifiers,
//! error codes, and helper functions that all bindings can rely on while the
//! remainder of the API surface is implemented in later steps.

use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::OnceLock;

/// Semantic version of the Rust core (mirrors `Cargo.toml`).
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Semantic version of the FFI surface. Bumps when ABI/contract changes.
pub const FFI_VERSION: &str = "0.1.0";

/// Error categories surfaced across the FFI boundary.
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum FfiErrorCategory {
    Ok = 0,
    Validation = 1,
    Persistence = 2,
    Currency = 3,
    Simulation = 4,
    Internal = 5,
}

impl From<FfiErrorCategory> for i32 {
    fn from(value: FfiErrorCategory) -> Self {
        value as i32
    }
}

/// Returns the core (Rust) semantic version as a C string.
#[no_mangle]
pub extern "C" fn ffi_core_version() -> *const c_char {
    static CORE: OnceLock<CString> = OnceLock::new();
    CORE.get_or_init(|| CString::new(CORE_VERSION).expect("static core version"))
        .as_ptr()
}

/// Returns the FFI interface semantic version as a C string.
#[no_mangle]
pub extern "C" fn ffi_version() -> *const c_char {
    static FFI: OnceLock<CString> = OnceLock::new();
    FFI.get_or_init(|| CString::new(FFI_VERSION).expect("static ffi version"))
        .as_ptr()
}

/// Helper that maps upcoming error values into categories.
pub fn classify_error(err: &crate::errors::LedgerError) -> FfiErrorCategory {
    use crate::errors::LedgerError;
    match err {
        LedgerError::InvalidInput(_) => FfiErrorCategory::Validation,
        LedgerError::InvalidRef(_) => FfiErrorCategory::Validation,
        LedgerError::Io(_) | LedgerError::Serde(_) => FfiErrorCategory::Persistence,
        LedgerError::Persistence(_) => FfiErrorCategory::Persistence,
    }
}

/// Placeholder opaque handle type. Later steps will replace this with the
/// actual session/ledger state wrapper.
#[repr(C)]
pub struct LedgerHandle {
    _private: [u8; 0],
}

/// Placeholder result handle type for future use.
#[repr(C)]
pub struct ResultHandle {
    _private: [u8; 0],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_versions() {
        unsafe {
            assert!(!ffi_core_version().is_null());
            assert!(!ffi_version().is_null());
        }
    }

    #[test]
    fn classifies_errors() {
        let err = crate::errors::LedgerError::InvalidInput("bad".into());
        assert!(matches!(classify_error(&err), FfiErrorCategory::Validation));
    }
}
