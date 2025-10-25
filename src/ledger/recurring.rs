use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use uuid::Uuid;

use super::{DateWindow, Transaction, TransactionStatus};
use crate::ledger::transaction::{Recurrence, RecurrenceMode, RecurrenceStatus};

const MAX_FORECAST_OCCURRENCES: usize = 1024;
const PENDING_WINDOW_DAYS: i64 = 7;
const SNAPSHOT_LOOKAHEAD_DAYS: i64 = 365 * 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledStatus {
    Overdue,
    Pending,
    Future,
}

impl ScheduledStatus {
    fn classify(scheduled: NaiveDate, reference: NaiveDate) -> ScheduledStatus {
        if scheduled < reference {
            return ScheduledStatus::Overdue;
        }
        let pending_cutoff = reference + Duration::days(PENDING_WINDOW_DAYS);
        if scheduled <= pending_cutoff {
            ScheduledStatus::Pending
        } else {
            ScheduledStatus::Future
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScheduledInstance {
    pub series_id: Uuid,
    pub template_id: Uuid,
    pub occurrence_index: u32,
    pub scheduled_date: NaiveDate,
    pub status: ScheduledStatus,
    pub exists_in_ledger: bool,
    pub transaction_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ForecastTransaction {
    pub transaction: Transaction,
    pub status: ScheduledStatus,
    pub occurrence_index: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ForecastTotals {
    pub generated: usize,
    pub projected_inflow: f64,
    pub projected_outflow: f64,
    pub net: f64,
}

impl ForecastTotals {
    fn from_transactions(transactions: &[ForecastTransaction]) -> Self {
        let mut totals = ForecastTotals::default();
        totals.generated = transactions.len();
        for item in transactions {
            let amount = item.transaction.budgeted_amount;
            if amount >= 0.0 {
                totals.projected_outflow += amount;
            } else {
                totals.projected_inflow += amount.abs();
            }
        }
        totals.net = totals.projected_inflow - totals.projected_outflow;
        totals
    }
}

#[derive(Debug, Clone)]
pub struct ForecastResult {
    pub window: DateWindow,
    pub reference_date: NaiveDate,
    pub instances: Vec<ScheduledInstance>,
    pub transactions: Vec<ForecastTransaction>,
    pub totals: ForecastTotals,
}

#[derive(Debug, Clone)]
pub struct RecurrenceSnapshot {
    pub series_id: Uuid,
    pub template_id: Uuid,
    pub start_date: NaiveDate,
    pub interval_label: String,
    pub next_due: Option<NaiveDate>,
    pub overdue: usize,
    pub pending: usize,
    pub status: RecurrenceStatus,
}

#[derive(Debug, Clone)]
pub struct SeriesMetadata {
    pub series_id: Uuid,
    pub last_generated: Option<NaiveDate>,
    pub last_completed: Option<NaiveDate>,
    pub next_due: Option<NaiveDate>,
    pub total_occurrences: u32,
}

#[derive(Clone)]
struct StateInfo {
    scheduled_date: NaiveDate,
    actual_date: Option<NaiveDate>,
}

#[derive(Clone)]
struct Occurrence<'a> {
    index: u32,
    scheduled_date: NaiveDate,
    transaction: Option<&'a Transaction>,
}

pub fn forecast_for_window(
    window: DateWindow,
    reference: NaiveDate,
    transactions: &[Transaction],
) -> ForecastResult {
    let series_map = collect_series_entries(transactions);
    let mut instances = Vec::new();
    let mut generated = Vec::new();

    for template in transactions.iter().filter(|t| t.recurrence.is_some()) {
        let recurrence = template.recurrence.as_ref().unwrap();
        let series_id = template.recurrence_series().unwrap_or(template.id);
        let mut entries = series_map
            .get(&series_id)
            .cloned()
            .unwrap_or_else(|| vec![template]);
        entries.sort_by_key(|txn| txn.scheduled_date);
        let (series_instances, series_generated) =
            project_series_in_window(template, recurrence, &entries, window, reference);
        instances.extend(series_instances);
        generated.extend(series_generated);
    }

    instances.sort_by_key(|inst| inst.scheduled_date);
    let totals = ForecastTotals::from_transactions(&generated);

    ForecastResult {
        window,
        reference_date: reference,
        instances,
        transactions: generated,
        totals,
    }
}

pub fn snapshot_recurrences(
    transactions: &[Transaction],
    reference: NaiveDate,
) -> Vec<RecurrenceSnapshot> {
    let series_map = collect_series_entries(transactions);
    let mut snapshots = Vec::new();
    let lookahead_end = reference + Duration::days(SNAPSHOT_LOOKAHEAD_DAYS);

    for template in transactions.iter().filter(|t| t.recurrence.is_some()) {
        let recurrence = template.recurrence.as_ref().unwrap();
        let series_id = template.recurrence_series().unwrap_or(template.id);
        let mut entries = series_map
            .get(&series_id)
            .cloned()
            .unwrap_or_else(|| vec![template]);
        entries.sort_by_key(|txn| txn.scheduled_date);
        let occurrences = build_occurrences(recurrence, &entries, lookahead_end);
        let mut overdue = 0usize;
        let mut pending = 0usize;
        let mut next_due = recurrence.next_scheduled;

        for occurrence in occurrences {
            let status = ScheduledStatus::classify(occurrence.scheduled_date, reference);
            match occurrence.transaction {
                Some(txn) => {
                    if txn.actual_date.is_none() {
                        match status {
                            ScheduledStatus::Overdue => overdue += 1,
                            ScheduledStatus::Pending => pending += 1,
                            ScheduledStatus::Future => {}
                        }
                        if next_due.is_none() && occurrence.scheduled_date >= reference {
                            next_due = Some(occurrence.scheduled_date);
                        }
                    }
                }
                None => {
                    if recurrence.status == RecurrenceStatus::Active {
                        match status {
                            ScheduledStatus::Overdue => overdue += 1,
                            ScheduledStatus::Pending => pending += 1,
                            ScheduledStatus::Future => {}
                        }
                        if next_due.is_none() && occurrence.scheduled_date >= reference {
                            next_due = Some(occurrence.scheduled_date);
                        }
                    }
                }
            }
        }

        snapshots.push(RecurrenceSnapshot {
            series_id,
            template_id: template.id,
            start_date: recurrence.start_date,
            interval_label: recurrence.interval.label(),
            next_due,
            overdue,
            pending,
            status: recurrence.status.clone(),
        });
    }

    snapshots.sort_by_key(|snap| (snap.next_due, Some(snap.template_id)));
    snapshots
}

pub fn rebuild_metadata(transactions: &[Transaction]) -> HashMap<Uuid, SeriesMetadata> {
    let mut states: HashMap<Uuid, Vec<StateInfo>> = HashMap::new();
    for txn in transactions {
        if let Some(series_id) = txn.recurrence_series() {
            states.entry(series_id).or_default().push(StateInfo {
                scheduled_date: txn.scheduled_date,
                actual_date: txn.actual_date,
            });
        }
    }
    for entry in states.values_mut() {
        entry.sort_by_key(|state| state.scheduled_date);
    }

    let mut metadata = HashMap::new();
    for template in transactions.iter().filter(|t| t.recurrence.is_some()) {
        let recurrence = template.recurrence.as_ref().unwrap();
        let series_id = template.recurrence_series().unwrap_or(template.id);
        if metadata.contains_key(&series_id) {
            continue;
        }
        let series_states = states.get(&series_id).cloned().unwrap_or_else(|| {
            vec![StateInfo {
                scheduled_date: template.scheduled_date,
                actual_date: template.actual_date,
            }]
        });
        let last_generated = series_states
            .iter()
            .map(|state| state.scheduled_date)
            .max()
            .or(Some(recurrence.start_date));
        let last_completed = series_states
            .iter()
            .filter_map(|state| state.actual_date)
            .max();
        let next_due = next_due_from_states(recurrence, &series_states);
        metadata.insert(
            series_id,
            SeriesMetadata {
                series_id,
                last_generated,
                last_completed,
                next_due,
                total_occurrences: series_states.len() as u32,
            },
        );
    }

    metadata
}

/// Builds concrete ledger transactions for any recurring occurrences scheduled on or
/// before the provided reference date that are missing from the ledger. The returned
/// transactions are detached from the recurrence definition so they represent
/// individual instances ready for persistence.
pub fn materialize_due_instances(
    reference: NaiveDate,
    transactions: &[Transaction],
) -> Vec<Transaction> {
    let mut creations = Vec::new();
    let limit_end = reference + Duration::days(1);
    let series_map = collect_series_entries(transactions);

    for template in transactions.iter().filter(|t| t.recurrence.is_some()) {
        let recurrence = template.recurrence.as_ref().unwrap();
        if recurrence.status != RecurrenceStatus::Active {
            continue;
        }
        let series_id = template.recurrence_series().unwrap_or(template.id);
        let entries = series_map
            .get(&series_id)
            .cloned()
            .unwrap_or_else(|| vec![template]);
        let occurrences = build_occurrences(recurrence, &entries, limit_end);
        for occurrence in occurrences {
            if occurrence.scheduled_date > reference {
                continue;
            }
            if occurrence.transaction.is_some() {
                continue;
            }
            let mut txn = template.clone();
            txn.id = Uuid::new_v4();
            txn.scheduled_date = occurrence.scheduled_date;
            txn.actual_date = None;
            txn.actual_amount = None;
            txn.status = TransactionStatus::Planned;
            txn.recurrence = None;
            txn.recurrence_series_id = Some(series_id);
            creations.push(txn);
            if creations.len() >= MAX_FORECAST_OCCURRENCES {
                break;
            }
        }
        if creations.len() >= MAX_FORECAST_OCCURRENCES {
            break;
        }
    }

    creations
}

fn collect_series_entries<'a>(
    transactions: &'a [Transaction],
) -> HashMap<Uuid, Vec<&'a Transaction>> {
    let mut map: HashMap<Uuid, Vec<&Transaction>> = HashMap::new();
    for txn in transactions {
        if let Some(series_id) = txn.recurrence_series() {
            map.entry(series_id).or_default().push(txn);
        }
    }
    map
}

fn project_series_in_window(
    template: &Transaction,
    recurrence: &Recurrence,
    entries: &[&Transaction],
    window: DateWindow,
    reference: NaiveDate,
) -> (Vec<ScheduledInstance>, Vec<ForecastTransaction>) {
    let occurrences = build_occurrences(recurrence, entries, window.end);
    let mut instances = Vec::new();
    let mut generated = Vec::new();
    let series_id = template.recurrence_series().unwrap_or(template.id);

    for occurrence in occurrences {
        if !window.contains(occurrence.scheduled_date) {
            continue;
        }
        match occurrence.transaction {
            Some(txn) => {
                if txn.actual_date.is_none() {
                    let status = ScheduledStatus::classify(occurrence.scheduled_date, reference);
                    instances.push(ScheduledInstance {
                        series_id,
                        template_id: template.id,
                        occurrence_index: occurrence.index,
                        scheduled_date: occurrence.scheduled_date,
                        status,
                        exists_in_ledger: true,
                        transaction_id: Some(txn.id),
                    });
                }
            }
            None => {
                if recurrence.status != RecurrenceStatus::Active {
                    continue;
                }
                let status = ScheduledStatus::classify(occurrence.scheduled_date, reference);
                let mut forecast = template.clone();
                forecast.id = Uuid::new_v4();
                forecast.scheduled_date = occurrence.scheduled_date;
                forecast.actual_date = None;
                forecast.actual_amount = None;
                forecast.status = TransactionStatus::Planned;
                forecast.recurrence_series_id = Some(series_id);
                generated.push(ForecastTransaction {
                    transaction: forecast,
                    status,
                    occurrence_index: occurrence.index,
                });
                instances.push(ScheduledInstance {
                    series_id,
                    template_id: template.id,
                    occurrence_index: occurrence.index,
                    scheduled_date: occurrence.scheduled_date,
                    status,
                    exists_in_ledger: false,
                    transaction_id: None,
                });
                if generated.len() >= MAX_FORECAST_OCCURRENCES {
                    break;
                }
            }
        }
    }

    (instances, generated)
}

fn build_occurrences<'a>(
    recurrence: &Recurrence,
    entries: &[&'a Transaction],
    limit_end: NaiveDate,
) -> Vec<Occurrence<'a>> {
    let mut result = Vec::new();
    if limit_end <= recurrence.start_date {
        return result;
    }

    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by_key(|txn| txn.scheduled_date);
    let mut iter = sorted_entries.into_iter().peekable();

    let mut occurrence_index = 0u32;
    let mut scheduled_date = recurrence.start_date;
    let mut guard = 0usize;

    while scheduled_date < limit_end && guard < MAX_FORECAST_OCCURRENCES {
        if !recurrence.allows_occurrence(occurrence_index, scheduled_date) {
            break;
        }
        if recurrence.is_exception(scheduled_date) {
            scheduled_date = recurrence.interval.next_date(scheduled_date);
            continue;
        }
        while let Some(next_txn) = iter.peek() {
            if next_txn.scheduled_date < scheduled_date {
                iter.next();
            } else {
                break;
            }
        }
        let txn = if let Some(next_txn) = iter.peek() {
            if next_txn.scheduled_date == scheduled_date {
                iter.next()
            } else {
                None
            }
        } else {
            None
        };
        result.push(Occurrence {
            index: occurrence_index,
            scheduled_date,
            transaction: txn,
        });
        let anchor = match recurrence.mode {
            RecurrenceMode::FixedSchedule => scheduled_date,
            RecurrenceMode::AfterLastPerformed => {
                txn.and_then(|t| t.actual_date).unwrap_or(scheduled_date)
            }
        };
        scheduled_date = recurrence.interval.next_date(anchor);
        occurrence_index += 1;
        guard += 1;
    }

    result
}

fn next_due_from_states(recurrence: &Recurrence, states: &[StateInfo]) -> Option<NaiveDate> {
    if recurrence.status == RecurrenceStatus::Completed {
        return None;
    }
    if states.is_empty() {
        return Some(recurrence.start_date);
    }
    let last_scheduled = states
        .iter()
        .map(|state| state.scheduled_date)
        .max()
        .unwrap_or(recurrence.start_date);
    let last_actual = states
        .iter()
        .rev()
        .find(|state| state.scheduled_date == last_scheduled)
        .and_then(|state| state.actual_date);
    let mut candidate = recurrence.next_occurrence(last_scheduled, last_actual);
    let mut attempts = 0usize;
    while recurrence.is_exception(candidate) {
        candidate = recurrence.interval.next_date(candidate);
        attempts += 1;
        if attempts >= MAX_FORECAST_OCCURRENCES {
            return None;
        }
    }
    if recurrence.allows_occurrence(states.len() as u32, candidate) {
        Some(candidate)
    } else {
        None
    }
}
