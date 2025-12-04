//! Services related to transaction recurrence maintenance.

use chrono::NaiveDate;
use uuid::Uuid;

use bufy_domain::{Ledger, Recurrence, RecurrenceStatus};

use crate::CoreError;

/// Provides safe helpers for modifying recurrence metadata on ledger transactions.
pub struct RecurrenceService;

impl RecurrenceService {
    /// Assigns or replaces the recurrence definition for a transaction.
    pub fn set_rule(
        ledger: &mut Ledger,
        transaction_id: Uuid,
        recurrence: Recurrence,
    ) -> Result<(), CoreError> {
        let txn = ledger
            .transaction_mut(transaction_id)
            .ok_or(CoreError::TransactionNotFound(transaction_id))?;
        txn.set_recurrence(Some(recurrence));
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        Ok(())
    }

    /// Clears any recurrence information associated with the transaction.
    pub fn clear_rule(ledger: &mut Ledger, transaction_id: Uuid) -> Result<bool, CoreError> {
        let txn = ledger
            .transaction_mut(transaction_id)
            .ok_or(CoreError::TransactionNotFound(transaction_id))?;
        let had_recurrence = txn.recurrence.is_some();
        txn.set_recurrence(None);
        txn.recurrence_series_id = None;
        if had_recurrence {
            ledger.refresh_recurrence_metadata();
            ledger.touch();
        }
        Ok(had_recurrence)
    }

    /// Updates the status of the recurrence.
    pub fn set_status(
        ledger: &mut Ledger,
        transaction_id: Uuid,
        status: RecurrenceStatus,
    ) -> Result<(), CoreError> {
        let txn = ledger
            .transaction_mut(transaction_id)
            .ok_or(CoreError::TransactionNotFound(transaction_id))?;
        let recurrence = txn
            .recurrence
            .as_mut()
            .ok_or_else(|| CoreError::InvalidOperation("transaction has no recurrence".into()))?;
        recurrence.status = status;
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        Ok(())
    }

    /// Adds a skipped date to the recurrence, returning whether it was newly added.
    pub fn skip_date(
        ledger: &mut Ledger,
        transaction_id: Uuid,
        date: NaiveDate,
    ) -> Result<bool, CoreError> {
        let txn = ledger
            .transaction_mut(transaction_id)
            .ok_or(CoreError::TransactionNotFound(transaction_id))?;
        let recurrence = txn
            .recurrence
            .as_mut()
            .ok_or_else(|| CoreError::InvalidOperation("transaction has no recurrence".into()))?;
        if recurrence.exceptions.contains(&date) {
            return Ok(false);
        }
        recurrence.exceptions.push(date);
        recurrence.exceptions.sort();
        ledger.refresh_recurrence_metadata();
        ledger.touch();
        Ok(true)
    }
}
