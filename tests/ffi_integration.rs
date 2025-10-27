#![cfg(feature = "ffi")]

use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;
use std::thread;
use tempfile::NamedTempFile;

type FfiStatus = i32;

type FfiVersionFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;
type FfiCreateFn = unsafe extern "C" fn(
    name: *const std::os::raw::c_char,
    out_handle: *mut *mut std::os::raw::c_void,
) -> FfiStatus;
type FfiSaveFn = unsafe extern "C" fn(
    handle: *mut std::os::raw::c_void,
    path: *const std::os::raw::c_char,
) -> FfiStatus;
type FfiLoadFn = unsafe extern "C" fn(
    path: *const std::os::raw::c_char,
    out_handle: *mut *mut std::os::raw::c_void,
) -> FfiStatus;
type FfiSnapshotFn = unsafe extern "C" fn(
    handle: *mut std::os::raw::c_void,
    out_json: *mut *mut std::os::raw::c_char,
) -> FfiStatus;
type FfiFreeFn = unsafe extern "C" fn(handle: *mut std::os::raw::c_void);
type FfiStringFreeFn = unsafe extern "C" fn(ptr: *mut std::os::raw::c_char);

type FfiLastErrorMessageFn =
    unsafe extern "C" fn(buffer: *mut std::os::raw::c_char, length: usize) -> FfiStatus;

type FfiLastErrorCategoryFn = unsafe extern "C" fn() -> FfiStatus;

fn lib_path() -> PathBuf {
    let exe = std::env::current_exe().expect("current exe");
    let debug_dir = exe.parent().and_then(|p| p.parent()).expect("debug dir");
    let mut path = debug_dir.to_path_buf();
    let libname = if cfg!(target_os = "macos") {
        "libbudget_core.dylib"
    } else if cfg!(target_os = "windows") {
        "budget_core.dll"
    } else {
        "libbudget_core.so"
    };
    path.push(libname);
    path
}

#[test]
fn dynamic_load_and_round_trip() {
    let lib = unsafe { Library::new(lib_path()) }.expect("load budget_core cdylib");

    unsafe {
        let ffi_version: Symbol<FfiVersionFn> = lib.get(b"ffi_version").unwrap();
        let version_c = ffi_version();
        assert!(!version_c.is_null());
        let version = CStr::from_ptr(version_c).to_str().unwrap();
        assert!(!version.is_empty());
    }

    unsafe {
        let ffi_create: Symbol<FfiCreateFn> = lib.get(b"ffi_ledger_create").unwrap();
        let ffi_save: Symbol<FfiSaveFn> = lib.get(b"ffi_ledger_save").unwrap();
        let ffi_load: Symbol<FfiLoadFn> = lib.get(b"ffi_ledger_load").unwrap();
        let ffi_snapshot: Symbol<FfiSnapshotFn> = lib.get(b"ffi_ledger_snapshot").unwrap();
        let ffi_free: Symbol<FfiFreeFn> = lib.get(b"ffi_ledger_free").unwrap();
        let ffi_string_free: Symbol<FfiStringFreeFn> = lib.get(b"ffi_string_free").unwrap();
        let ffi_last_error_msg: Symbol<FfiLastErrorMessageFn> =
            lib.get(b"ffi_last_error_message").unwrap();
        let ffi_last_error_category: Symbol<FfiLastErrorCategoryFn> =
            lib.get(b"ffi_last_error_category").unwrap();

        let name = CString::new("FFI Demo").unwrap();
        let mut handle: *mut std::os::raw::c_void = ptr::null_mut();
        assert_eq!(ffi_create(name.as_ptr(), &mut handle), 0);
        assert!(!handle.is_null());

        // Snapshot to ensure ledger was created.
        let mut json_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        assert_eq!(ffi_snapshot(handle, &mut json_ptr), 0);
        let snapshot = CStr::from_ptr(json_ptr).to_str().unwrap().to_owned();
        assert!(snapshot.contains("FFI Demo"));
        ffi_string_free(json_ptr);

        // Save to temporary file.
        let tmp = NamedTempFile::new().unwrap();
        let path_c = CString::new(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(ffi_save(handle, path_c.as_ptr()), 0);
        ffi_free(handle);

        // Reload via FFI.
        let mut handle2: *mut std::os::raw::c_void = ptr::null_mut();
        assert_eq!(ffi_load(path_c.as_ptr(), &mut handle2), 0);
        assert!(!handle2.is_null());

        // Snapshot reloaded ledger.
        let mut json_ptr2: *mut std::os::raw::c_char = ptr::null_mut();
        assert_eq!(ffi_snapshot(handle2, &mut json_ptr2), 0);
        let snap2 = CStr::from_ptr(json_ptr2).to_str().unwrap();
        assert!(snap2.contains("FFI Demo"));
        ffi_string_free(json_ptr2);
        ffi_free(handle2);

        // Trigger validation error (null path) and read it back.
        let mut handle3: *mut std::os::raw::c_void = ptr::null_mut();
        let status = ffi_load(ptr::null(), &mut handle3);
        assert_ne!(status, 0);
        let category = ffi_last_error_category();
        assert_eq!(category, 1); // Validation
        let mut buffer = vec![0i8; 128];
        let written = ffi_last_error_msg(buffer.as_mut_ptr(), buffer.len());
        assert!(written > 0);
        let message = CStr::from_ptr(buffer.as_ptr()).to_str().unwrap();
        assert!(message.contains("path pointer was null"));
    }
}

#[test]
fn ffi_parallel_snapshots_are_thread_safe() {
    let lib = unsafe { Library::new(lib_path()) }.expect("load budget_core cdylib");

    let ffi_create: Symbol<FfiCreateFn> =
        unsafe { lib.get(b"ffi_ledger_create").unwrap() };
    let ffi_snapshot: Symbol<FfiSnapshotFn> =
        unsafe { lib.get(b"ffi_ledger_snapshot").unwrap() };
    let ffi_free: Symbol<FfiFreeFn> =
        unsafe { lib.get(b"ffi_ledger_free").unwrap() };
    let ffi_string_free: Symbol<FfiStringFreeFn> =
        unsafe { lib.get(b"ffi_string_free").unwrap() };

    let create_fn: FfiCreateFn = *ffi_create;
    let snapshot_fn: FfiSnapshotFn = *ffi_snapshot;
    let free_fn: FfiFreeFn = *ffi_free;
    let string_free_fn: FfiStringFreeFn = *ffi_string_free;

    let name = CString::new("FFI Concurrent").unwrap();
    let mut handle: *mut std::os::raw::c_void = ptr::null_mut();
    assert_eq!(unsafe { create_fn(name.as_ptr(), &mut handle) }, 0);
    assert!(!handle.is_null());

    let handle_addr = handle as usize;
    let mut threads = Vec::new();
    for _ in 0..8 {
        let snapshot_fn = snapshot_fn;
        let string_free_fn = string_free_fn;
        let handle_addr = handle_addr;
        threads.push(thread::spawn(move || {
            let handle_ptr = handle_addr as *mut std::os::raw::c_void;
            for _ in 0..16 {
                let mut json_ptr: *mut std::os::raw::c_char = ptr::null_mut();
                let status = unsafe { snapshot_fn(handle_ptr, &mut json_ptr) };
                assert_eq!(status, 0);
                assert!(!json_ptr.is_null());
                unsafe { string_free_fn(json_ptr) };
            }
        }));
    }

    for th in threads {
        th.join().expect("thread join");
    }

    unsafe { free_fn(handle) };
}
