#![allow(dead_code)]

//! Foreign Function Interface (FFI) module.
//! Provides stable C-compatible entry points that allow Swift, Kotlin, and C#
//! clients to interact with the budgeting core. Phase 9 will expand this module
//! with the full API surface; at this stage we expose version metadata, error
//! handling helpers, and core ledger lifecycle functions (create/load/save/
//! snapshot) to validate threading and ownership semantics.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic;
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use crate::{
    errors::LedgerError,
    ledger::{BudgetPeriod, Ledger},
    storage::json_backend::{
        load_ledger_from_path as load_ledger_from_file, save_ledger_to_path as save_ledger_to_file,
    },
};

const CORE_VERSION_STR: &str = env!("CARGO_PKG_VERSION");
const FFI_VERSION_STR: &str = "0.1.0";

pub const CORE_VERSION: &str = CORE_VERSION_STR;
pub const FFI_VERSION: &str = FFI_VERSION_STR;

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

thread_local! {
    static LAST_ERROR: RefCell<Option<(FfiErrorCategory, String)>> = RefCell::new(None);
}

fn clear_error() {
    LAST_ERROR.with(|cell| {
        cell.borrow_mut().take();
    });
}

fn set_error(category: FfiErrorCategory, message: impl Into<String>) -> i32 {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some((category, message.into()));
    });
    category.into()
}

fn ok() -> i32 {
    clear_error();
    FfiErrorCategory::Ok.into()
}

#[no_mangle]
pub extern "C" fn ffi_core_version() -> *const c_char {
    static CORE: OnceLock<CString> = OnceLock::new();
    CORE.get_or_init(|| CString::new(CORE_VERSION_STR).expect("static core version"))
        .as_ptr()
}

#[no_mangle]
pub extern "C" fn ffi_version() -> *const c_char {
    static FFI: OnceLock<CString> = OnceLock::new();
    FFI.get_or_init(|| CString::new(FFI_VERSION_STR).expect("static ffi version"))
        .as_ptr()
}

#[no_mangle]
pub extern "C" fn ffi_last_error_category() -> i32 {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(cat, _)| (*cat).into())
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn ffi_last_error_message(buffer: *mut c_char, length: usize) -> i32 {
    if buffer.is_null() || length == 0 {
        return -1;
    }
    LAST_ERROR.with(|cell| {
        if let Some((_, ref msg)) = *cell.borrow() {
            let bytes = msg.as_bytes();
            let max_copy = bytes.len().min(length.saturating_sub(1));
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buffer as *mut u8, max_copy);
                *buffer.add(max_copy) = 0;
            }
            max_copy as i32
        } else {
            unsafe {
                *buffer = 0;
            }
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn ffi_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

#[repr(C)]
pub struct LedgerHandle {
    inner: Arc<Mutex<LedgerSession>>,
}

impl LedgerHandle {
    fn new(ledger: Ledger) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LedgerSession { ledger })),
        }
    }
}

struct LedgerSession {
    ledger: Ledger,
}

#[repr(C, align(1))]
pub struct ResultHandle {
    _private: [u8; 0],
}

fn classify_error(err: &LedgerError) -> FfiErrorCategory {
    match err {
        LedgerError::InvalidInput(_) | LedgerError::InvalidRef(_) => FfiErrorCategory::Validation,
        LedgerError::Io(_) | LedgerError::Serde(_) | LedgerError::Persistence(_) => {
            FfiErrorCategory::Persistence
        }
    }
}

fn c_str_to_string(ptr: *const c_char, field: &str) -> Result<String, i32> {
    if ptr.is_null() {
        return Err(set_error(
            FfiErrorCategory::Validation,
            format!("{} pointer was null", field),
        ));
    }
    let c_str = unsafe { CStr::from_ptr(ptr) };
    match c_str.to_str() {
        Ok(s) => Ok(s.to_owned()),
        Err(_) => Err(set_error(
            FfiErrorCategory::Validation,
            format!("{} contained invalid UTF-8", field),
        )),
    }
}

fn with_session<T, F>(handle: *mut LedgerHandle, f: F) -> Result<T, i32>
where
    F: FnOnce(&mut LedgerSession) -> Result<T, LedgerError>,
{
    if handle.is_null() {
        return Err(set_error(
            FfiErrorCategory::Validation,
            "ledger handle was null",
        ));
    }
    let arc = unsafe { (*handle).inner.clone() };
    let mut guard = match arc.lock() {
        Ok(g) => g,
        Err(_) => {
            return Err(set_error(
                FfiErrorCategory::Internal,
                "ledger lock poisoned",
            ))
        }
    };

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| f(&mut guard)));
    match result {
        Ok(Ok(value)) => {
            clear_error();
            Ok(value)
        }
        Ok(Err(err)) => Err(set_error(classify_error(&err), err.to_string())),
        Err(_) => Err(set_error(
            FfiErrorCategory::Internal,
            "panic across FFI boundary",
        )),
    }
}

#[no_mangle]
pub extern "C" fn ffi_ledger_create(
    name: *const c_char,
    out_handle: *mut *mut LedgerHandle,
) -> i32 {
    if out_handle.is_null() {
        return set_error(FfiErrorCategory::Validation, "out_handle was null");
    }
    let name = match c_str_to_string(name, "name") {
        Ok(n) => n,
        Err(code) => return code,
    };
    let ledger = Ledger::new(name, BudgetPeriod::default());
    let handle = Box::new(LedgerHandle::new(ledger));
    unsafe {
        *out_handle = Box::into_raw(handle);
    }
    ok()
}

#[no_mangle]
pub extern "C" fn ffi_ledger_load(path: *const c_char, out_handle: *mut *mut LedgerHandle) -> i32 {
    if out_handle.is_null() {
        return set_error(FfiErrorCategory::Validation, "out_handle was null");
    }
    let path = match c_str_to_string(path, "path") {
        Ok(p) => p,
        Err(code) => return code,
    };
    match load_ledger_from_file(Path::new(&path)) {
        Ok(ledger) => {
            let handle = Box::new(LedgerHandle::new(ledger));
            unsafe {
                *out_handle = Box::into_raw(handle);
            }
            ok()
        }
        Err(err) => set_error(classify_error(&err), err.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn ffi_ledger_save(handle: *mut LedgerHandle, path: *const c_char) -> i32 {
    let path = match c_str_to_string(path, "path") {
        Ok(p) => p,
        Err(code) => return code,
    };
    match with_session(handle, |session| {
        save_ledger_to_file(&session.ledger, Path::new(&path))
    }) {
        Ok(()) => ok(),
        Err(code) => code,
    }
}

#[no_mangle]
pub extern "C" fn ffi_ledger_snapshot(
    handle: *mut LedgerHandle,
    out_json: *mut *mut c_char,
) -> i32 {
    if out_json.is_null() {
        return set_error(FfiErrorCategory::Validation, "out_json was null");
    }
    let json = match with_session(handle, |session| {
        Ok(serde_json::to_string_pretty(&session.ledger)?)
    }) {
        Ok(j) => j,
        Err(code) => return code,
    };
    match CString::new(json) {
        Ok(cstr) => {
            unsafe {
                *out_json = cstr.into_raw();
            }
            ok()
        }
        Err(_) => set_error(
            FfiErrorCategory::Internal,
            "failed to create CString from JSON",
        ),
    }
}

#[no_mangle]
pub extern "C" fn ffi_ledger_free(handle: *mut LedgerHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[cfg(all(test, feature = "ffi"))]
mod ffi_runtime_tests {
    use super::*;
    use std::ffi::{CStr, CString};
    use tempfile::NamedTempFile;

    #[test]
    fn create_snapshot_and_free() {
        let name = CString::new("Demo").unwrap();
        let mut handle: *mut LedgerHandle = ptr::null_mut();
        assert_eq!(ffi_ledger_create(name.as_ptr(), &mut handle), 0);
        assert!(!handle.is_null());

        let mut json_ptr: *mut c_char = ptr::null_mut();
        assert_eq!(ffi_ledger_snapshot(handle, &mut json_ptr), 0);
        let json = unsafe { CStr::from_ptr(json_ptr) }.to_str().unwrap();
        assert!(json.contains("\"name\": \"Demo\""));
        ffi_string_free(json_ptr);

        ffi_ledger_free(handle);
    }

    #[test]
    fn save_and_reload_round_trip() {
        let name = CString::new("RoundTrip").unwrap();
        let mut handle: *mut LedgerHandle = ptr::null_mut();
        assert_eq!(ffi_ledger_create(name.as_ptr(), &mut handle), 0);

        let tmp = NamedTempFile::new().unwrap();
        let path_c = CString::new(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(ffi_ledger_save(handle, path_c.as_ptr()), 0);
        ffi_ledger_free(handle);

        let mut handle2: *mut LedgerHandle = ptr::null_mut();
        assert_eq!(ffi_ledger_load(path_c.as_ptr(), &mut handle2), 0);
        assert!(!handle2.is_null());
        ffi_ledger_free(handle2);
    }
}
