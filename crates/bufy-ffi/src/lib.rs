//! bufy-ffi
//!
//! Minimal FFI surface that exposes selected bufy-core APIs for external clients.

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_double, c_int},
    ptr,
};

use chrono::{Datelike, Utc};
use uuid::Uuid;

use bufy_core::{api_add_account, api_create_ledger, api_ledger_summary, CoreError};
use bufy_domain::{
    account::AccountKind,
    common::{TimeInterval, TimeUnit},
    ledger::BudgetScope,
    Ledger, LedgerBudgetPeriod,
};

/// Opaque pointer used by external callers to hold ledger state.
#[repr(C)]
pub struct LedgerHandle {
    inner: Ledger,
}

impl LedgerHandle {
    fn new(ledger: Ledger) -> *mut Self {
        Box::into_raw(Box::new(Self { inner: ledger }))
    }
}

/// Simple budgeting snapshot exposed over FFI.
#[repr(C)]
pub struct FfiLedgerSummary {
    pub window_start_year: i32,
    pub window_start_month: i32,
    pub window_start_day: i32,
    pub window_end_year: i32,
    pub window_end_month: i32,
    pub window_end_day: i32,
    pub scope: c_int,
    pub budgeted_total: c_double,
    pub actual_total: c_double,
    pub remaining_total: c_double,
    pub variance_total: c_double,
    pub incomplete_transactions: c_int,
    pub orphaned_transactions: c_int,
}

#[no_mangle]
pub extern "C" fn bufy_ledger_create(
    name: *const c_char,
    period_code: c_int,
    out_error: *mut *mut c_char,
) -> *mut LedgerHandle {
    clear_error(out_error);
    let ledger_name = match unsafe { c_string_argument(name) } {
        Ok(value) => value,
        Err(err) => {
            unsafe {
                write_core_error(out_error, err);
            }
            return ptr::null_mut();
        }
    };

    let period = ledger_period_from_code(period_code);
    let ledger = api_create_ledger(ledger_name, period);
    LedgerHandle::new(ledger)
}

#[no_mangle]
pub extern "C" fn bufy_ledger_free(handle: *mut LedgerHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub extern "C" fn bufy_ledger_add_account(
    handle: *mut LedgerHandle,
    name: *const c_char,
    kind_code: c_int,
    category_id: *const c_char,
    out_account_id: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> c_int {
    clear_error(out_error);
    if handle.is_null() {
        unsafe {
            write_error(out_error, "ledger handle is null");
        }
        return 1;
    }
    let ledger = unsafe { &mut (*handle).inner };
    let account_name = match unsafe { c_string_argument(name) } {
        Ok(value) => value,
        Err(err) => {
            unsafe {
                write_core_error(out_error, err);
            }
            return 2;
        }
    };

    let category = match unsafe { parse_optional_uuid(category_id) } {
        Ok(value) => value,
        Err(err) => {
            unsafe {
                write_core_error(out_error, err);
            }
            return 3;
        }
    };

    let kind = account_kind_from_code(kind_code);

    match api_add_account(ledger, account_name, kind, category) {
        Ok(account_id) => {
            unsafe {
                write_string(out_account_id, account_id.to_string());
            }
            0
        }
        Err(err) => {
            unsafe {
                write_core_error(out_error, err);
            }
            4
        }
    }
}

#[no_mangle]
pub extern "C" fn bufy_ledger_get_summary(
    handle: *const LedgerHandle,
    out_summary: *mut FfiLedgerSummary,
    out_error: *mut *mut c_char,
) -> c_int {
    clear_error(out_error);
    if handle.is_null() || out_summary.is_null() {
        unsafe {
            write_error(out_error, "ledger handle or output summary is null");
        }
        return 1;
    }

    let ledger = unsafe { &(*handle).inner };
    let reference = Utc::now().date_naive();
    let summary = api_ledger_summary(ledger, reference);

    unsafe {
        (*out_summary).window_start_year = summary.window_start.year();
        (*out_summary).window_start_month = summary.window_start.month() as i32;
        (*out_summary).window_start_day = summary.window_start.day() as i32;
        (*out_summary).window_end_year = summary.window_end.year();
        (*out_summary).window_end_month = summary.window_end.month() as i32;
        (*out_summary).window_end_day = summary.window_end.day() as i32;
        (*out_summary).scope = scope_to_code(summary.scope);
        (*out_summary).budgeted_total = summary.budgeted_total;
        (*out_summary).actual_total = summary.actual_total;
        (*out_summary).remaining_total = summary.remaining_total;
        (*out_summary).variance_total = summary.variance_total;
        (*out_summary).incomplete_transactions = summary.incomplete_transactions as c_int;
        (*out_summary).orphaned_transactions = summary.orphaned_transactions as c_int;
    }

    0
}

fn ledger_period_from_code(code: c_int) -> LedgerBudgetPeriod {
    match code {
        0 => LedgerBudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Day,
        }),
        1 => LedgerBudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Week,
        }),
        2 => LedgerBudgetPeriod::monthly(),
        3 => LedgerBudgetPeriod(TimeInterval {
            every: 1,
            unit: TimeUnit::Year,
        }),
        _ => LedgerBudgetPeriod::monthly(),
    }
}

fn account_kind_from_code(code: c_int) -> AccountKind {
    match code {
        0 => AccountKind::Bank,
        1 => AccountKind::Cash,
        2 => AccountKind::Savings,
        3 => AccountKind::ExpenseDestination,
        4 => AccountKind::IncomeSource,
        _ => AccountKind::Unknown,
    }
}

fn scope_to_code(scope: BudgetScope) -> c_int {
    match scope {
        BudgetScope::Past => 0,
        BudgetScope::Current => 1,
        BudgetScope::Future => 2,
        BudgetScope::Custom => 3,
    }
}

fn clear_error(out_error: *mut *mut c_char) {
    if out_error.is_null() {
        return;
    }
    unsafe {
        *out_error = ptr::null_mut();
    }
}

unsafe fn write_error(out_error: *mut *mut c_char, message: &str) {
    if out_error.is_null() {
        return;
    }
    if let Ok(cstring) = CString::new(message) {
        *out_error = cstring.into_raw();
    }
}

unsafe fn write_core_error(out_error: *mut *mut c_char, err: CoreError) {
    write_error(out_error, &err.to_string());
}

unsafe fn write_string(target: *mut *mut c_char, value: String) {
    if target.is_null() {
        return;
    }
    if let Ok(cstring) = CString::new(value) {
        *target = cstring.into_raw();
    }
}

unsafe fn c_string_argument(ptr: *const c_char) -> Result<String, CoreError> {
    if ptr.is_null() {
        return Err(CoreError::InvalidOperation(
            "null string pointer received".into(),
        ));
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map(|s| s.to_string())
        .map_err(|err| CoreError::InvalidOperation(err.to_string()))
}

unsafe fn parse_optional_uuid(ptr: *const c_char) -> Result<Option<Uuid>, CoreError> {
    if ptr.is_null() {
        return Ok(None);
    }
    let raw = c_string_argument(ptr)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Uuid::parse_str(trimmed)
        .map(Some)
        .map_err(|err| CoreError::Validation(format!("invalid UUID: {err}")))
}
