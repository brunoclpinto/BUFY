//! Stable, public-facing helpers that wrap the internal service layer.
//!
//! This module exposes a simplified API that other frontends (CLI, GUI, FFI)
//! can rely on without depending on the entire service surface area.

use chrono::NaiveDate;
use uuid::Uuid;

use bufy_domain::{
    account::{Account, AccountKind},
    ledger::BudgetScope,
    transaction::Transaction,
    Ledger, LedgerBudgetPeriod,
};

use crate::{
    account_service::AccountService, budget_service::BudgetService, ledger_service::LedgerService,
    transaction_service::TransactionService, CoreError,
};

/// Summarized budgeting totals for a ledger window.
#[derive(Debug, Clone)]
pub struct ApiLedgerSummary {
    pub scope: BudgetScope,
    pub window_start: NaiveDate,
    pub window_end: NaiveDate,
    pub budgeted_total: f64,
    pub actual_total: f64,
    pub remaining_total: f64,
    pub variance_total: f64,
    pub incomplete_transactions: usize,
    pub orphaned_transactions: usize,
}

/// Creates a new ledger with the supplied name and budgeting period.
pub fn api_create_ledger(name: impl Into<String>, period: LedgerBudgetPeriod) -> Ledger {
    LedgerService::create(name, period)
}

/// Adds an account to the provided ledger and returns its identifier.
pub fn api_add_account(
    ledger: &mut Ledger,
    name: impl Into<String>,
    kind: AccountKind,
    category_id: Option<Uuid>,
) -> Result<Uuid, CoreError> {
    let mut account = Account::new(name, kind);
    account.category_id = category_id;
    let account_id = account.id;
    AccountService::add(ledger, account)?;
    Ok(account_id)
}

/// Adds a transaction to the ledger and returns the transaction identifier.
#[allow(clippy::too_many_arguments)]
pub fn api_add_transaction(
    ledger: &mut Ledger,
    from_account: Uuid,
    to_account: Uuid,
    category_id: Option<Uuid>,
    scheduled_date: NaiveDate,
    budgeted_amount: f64,
    notes: Option<String>,
) -> Result<Uuid, CoreError> {
    let mut transaction = Transaction::new(
        from_account,
        to_account,
        category_id,
        scheduled_date,
        budgeted_amount,
    );
    transaction.notes = notes;
    TransactionService::add(ledger, transaction)
}

/// Marks the transaction identified by `txn_id` as completed.
pub fn api_complete_transaction(
    ledger: &mut Ledger,
    txn_id: Uuid,
    actual_date: NaiveDate,
    actual_amount: f64,
) -> Result<(), CoreError> {
    TransactionService::update(ledger, txn_id, |txn| {
        txn.mark_completed(actual_date, actual_amount);
    })
}

/// Provides a simplified ledger summary for the budgeting period that
/// contains `reference_date`.
pub fn api_ledger_summary(ledger: &Ledger, reference_date: NaiveDate) -> ApiLedgerSummary {
    let summary = BudgetService::summarize_period_containing(ledger, reference_date);
    let totals = summary.totals;
    ApiLedgerSummary {
        scope: summary.scope,
        window_start: summary.window.start,
        window_end: summary.window.end,
        budgeted_total: totals.budgeted,
        actual_total: totals.real,
        remaining_total: totals.remaining,
        variance_total: totals.variance,
        incomplete_transactions: summary.incomplete_transactions,
        orphaned_transactions: summary.orphaned_transactions,
    }
}
