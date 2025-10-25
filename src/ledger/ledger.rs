use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use uuid::Uuid;

use super::{
    account::Account,
    category::Category,
    time_interval::{TimeInterval, TimeUnit},
    transaction::Transaction,
};
use crate::errors::LedgerError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DateWindow {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateWindow {
    pub fn new(start: NaiveDate, end: NaiveDate) -> Result<Self, LedgerError> {
        if end <= start {
            return Err(LedgerError::InvalidInput(
                "window end must be after start".into(),
            ));
        }
        Ok(Self { start, end })
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date < self.end
    }

    pub fn scope(&self, reference: NaiveDate) -> BudgetScope {
        if self.contains(reference) {
            BudgetScope::Current
        } else if self.end <= reference {
            BudgetScope::Past
        } else if self.start > reference {
            BudgetScope::Future
        } else {
            BudgetScope::Custom
        }
    }

    pub fn shift(&self, interval: &TimeInterval, steps: i32) -> Self {
        let new_start = interval.add_to(self.start, steps);
        let new_end = interval.add_to(self.end, steps);
        Self {
            start: new_start,
            end: new_end,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetScope {
    Past,
    Current,
    Future,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetTotals {
    pub budgeted: f64,
    pub real: f64,
    pub remaining: f64,
    pub variance: f64,
    pub percent_used: Option<f64>,
    pub status: BudgetStatus,
    pub incomplete: bool,
}

impl BudgetTotals {
    fn from_parts(budgeted: f64, real: f64, incomplete: bool) -> Self {
        let remaining = budgeted - real;
        let variance = real - budgeted;
        let percent_used = if budgeted.abs() > f64::EPSILON {
            Some((real / budgeted) * 100.0)
        } else if real.abs() > f64::EPSILON {
            Some(100.0)
        } else {
            None
        };
        let status = if incomplete {
            BudgetStatus::Incomplete
        } else if budgeted.abs() < f64::EPSILON && real.abs() < f64::EPSILON {
            BudgetStatus::Empty
        } else {
            match real.partial_cmp(&budgeted).unwrap_or(Ordering::Equal) {
                Ordering::Greater => BudgetStatus::OverBudget,
                Ordering::Less => BudgetStatus::UnderBudget,
                Ordering::Equal => BudgetStatus::OnTrack,
            }
        };
        Self {
            budgeted,
            real,
            remaining,
            variance,
            percent_used,
            status,
            incomplete,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetStatus {
    OnTrack,
    OverBudget,
    UnderBudget,
    Empty,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBudget {
    pub category_id: Option<Uuid>,
    pub name: String,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBudget {
    pub account_id: Uuid,
    pub name: String,
    pub totals: BudgetTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummary {
    pub scope: BudgetScope,
    pub window: DateWindow,
    pub totals: BudgetTotals,
    pub per_category: Vec<CategoryBudget>,
    pub per_account: Vec<AccountBudget>,
    pub orphaned_transactions: usize,
    pub incomplete_transactions: usize,
}

const CURRENT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub id: Uuid,
    pub name: String,
    pub budget_period: BudgetPeriod,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default = "Ledger::schema_version_default")]
    pub schema_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetPeriod(pub TimeInterval);

impl BudgetPeriod {
    pub fn monthly() -> Self {
        Self(TimeInterval {
            every: 1,
            unit: TimeUnit::Month,
        })
    }
}

impl Default for BudgetPeriod {
    fn default() -> Self {
        Self::monthly()
    }
}

impl Ledger {
    pub fn new(name: impl Into<String>, budget_period: BudgetPeriod) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            budget_period,
            accounts: Vec::new(),
            categories: Vec::new(),
            transactions: Vec::new(),
            created_at: now,
            updated_at: now,
            schema_version: CURRENT_SCHEMA_VERSION,
        }
    }

    pub fn add_account(&mut self, account: Account) -> Uuid {
        let id = account.id;
        self.accounts.push(account);
        self.touch();
        id
    }

    pub fn add_category(&mut self, category: Category) -> Uuid {
        let id = category.id;
        self.categories.push(category);
        self.touch();
        id
    }

    pub fn add_transaction(&mut self, transaction: Transaction) -> Uuid {
        let id = transaction.id;
        self.transactions.push(transaction);
        self.touch();
        id
    }

    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    pub fn account(&self, id: Uuid) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn schema_version_default() -> u8 {
        CURRENT_SCHEMA_VERSION
    }

    pub fn summarize_current_period(&self) -> BudgetSummary {
        let today = Utc::now().date_naive();
        self.summarize_period_containing(today)
    }

    pub fn summarize_period_containing(&self, date: NaiveDate) -> BudgetSummary {
        let window = self.budget_window_containing(date);
        let scope = window.scope(date);
        self.summarize_window(window, scope, None)
    }

    pub fn summarize_period_offset(&self, reference: NaiveDate, offset: i32) -> BudgetSummary {
        let base = self.budget_window_containing(reference);
        let shifted = base.shift(&self.budget_period.0, offset);
        let scope = shifted.scope(reference);
        self.summarize_window(shifted, scope, None)
    }

    pub fn summaries_before(&self, reference: NaiveDate, periods: usize) -> Vec<BudgetSummary> {
        (1..=periods)
            .map(|idx| self.summarize_period_offset(reference, -(idx as i32)))
            .collect()
    }

    pub fn summaries_after(&self, reference: NaiveDate, periods: usize) -> Vec<BudgetSummary> {
        (1..=periods)
            .map(|idx| self.summarize_period_offset(reference, idx as i32))
            .collect()
    }

    pub fn summarize_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<BudgetSummary, LedgerError> {
        let window = DateWindow::new(start, end)?;
        Ok(self.summarize_window(window, BudgetScope::Custom, None))
    }

    pub fn summarize_range_with_transactions(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        transactions: &[Transaction],
    ) -> Result<BudgetSummary, LedgerError> {
        let window = DateWindow::new(start, end)?;
        Ok(self.summarize_window(window, BudgetScope::Custom, Some(transactions)))
    }

    fn summarize_window(
        &self,
        window: DateWindow,
        scope: BudgetScope,
        tx_override: Option<&[Transaction]>,
    ) -> BudgetSummary {
        let txs = tx_override.unwrap_or(&self.transactions);
        let mut totals_acc = Accumulator::default();
        let mut category_map: HashMap<Option<Uuid>, Accumulator> = HashMap::new();
        let mut account_map: HashMap<Uuid, Accumulator> = HashMap::new();
        let mut orphaned = 0usize;
        let mut incomplete_transactions = 0usize;

        let category_lookup: HashMap<Uuid, &Category> =
            self.categories.iter().map(|c| (c.id, c)).collect();
        let account_lookup: HashMap<Uuid, &Account> =
            self.accounts.iter().map(|a| (a.id, a)).collect();

        for txn in txs {
            let budget_in = window.contains(txn.scheduled_date);
            let actual_in = txn
                .actual_date
                .map(|date| window.contains(date))
                .unwrap_or(false);
            let actual_amount = txn.actual_amount;

            let mut txn_incomplete = false;

            if budget_in {
                totals_acc.add_budgeted(txn.budgeted_amount);
            }
            if actual_in {
                if let Some(amount) = actual_amount {
                    totals_acc.add_real(amount);
                } else {
                    totals_acc.missing_real = true;
                    txn_incomplete = true;
                }
            }
            if budget_in && txn.actual_amount.is_none() {
                totals_acc.missing_real = true;
                txn_incomplete = true;
            }
            if actual_in && !budget_in {
                totals_acc.missing_budget = true;
                txn_incomplete = true;
            }

            let cat_entry = category_map.entry(txn.category_id).or_default();
            if budget_in {
                cat_entry.add_budgeted(txn.budgeted_amount);
            }
            if actual_in {
                if let Some(amount) = actual_amount {
                    cat_entry.add_real(amount);
                } else {
                    cat_entry.missing_real = true;
                }
            }
            if actual_in && !budget_in {
                cat_entry.missing_budget = true;
            }
            if budget_in && txn.actual_amount.is_none() {
                cat_entry.missing_real = true;
            }

            let account_entry = account_map.entry(txn.from_account).or_default();
            if budget_in {
                account_entry.add_budgeted(txn.budgeted_amount);
            }
            if actual_in {
                if let Some(amount) = actual_amount {
                    account_entry.add_real(amount);
                } else {
                    account_entry.missing_real = true;
                }
            }
            if actual_in && !budget_in {
                account_entry.missing_budget = true;
            }
            if budget_in && txn.actual_amount.is_none() {
                account_entry.missing_real = true;
            }

            if !account_lookup.contains_key(&txn.from_account)
                || txn
                    .category_id
                    .map(|id| !category_lookup.contains_key(&id))
                    .unwrap_or(false)
            {
                orphaned += 1;
            }

            if txn_incomplete {
                incomplete_transactions += 1;
            }
        }

        let totals = BudgetTotals::from_parts(
            totals_acc.budgeted,
            totals_acc.real,
            totals_acc.is_incomplete(),
        );

        let mut per_category: Vec<CategoryBudget> = category_map
            .into_iter()
            .map(|(category_id, acc)| {
                let name = match category_id {
                    Some(id) => match category_lookup.get(&id) {
                        Some(cat) => cat.name.clone(),
                        None => "Unknown Category".into(),
                    },
                    None => "Uncategorized".into(),
                };
                CategoryBudget {
                    category_id,
                    name,
                    totals: BudgetTotals::from_parts(acc.budgeted, acc.real, acc.is_incomplete()),
                }
            })
            .collect();
        per_category.sort_by(|a, b| a.name.cmp(&b.name));

        let mut per_account: Vec<AccountBudget> = account_map
            .into_iter()
            .map(|(account_id, acc)| {
                let name = account_lookup
                    .get(&account_id)
                    .map(|acct| acct.name.clone())
                    .unwrap_or_else(|| "Unknown Account".into());
                AccountBudget {
                    account_id,
                    name,
                    totals: BudgetTotals::from_parts(acc.budgeted, acc.real, acc.is_incomplete()),
                }
            })
            .collect();
        per_account.sort_by(|a, b| a.name.cmp(&b.name));

        BudgetSummary {
            scope,
            window,
            totals,
            per_category,
            per_account,
            orphaned_transactions: orphaned,
            incomplete_transactions,
        }
    }

    fn budget_anchor_date(&self) -> NaiveDate {
        let base = self
            .transactions
            .iter()
            .map(|t| t.scheduled_date)
            .min()
            .unwrap_or_else(|| self.created_at.date_naive());
        self.budget_period.0.normalize_anchor(base)
    }

    fn budget_window_containing(&self, reference: NaiveDate) -> DateWindow {
        let anchor = self.budget_anchor_date();
        let start = self.budget_period.0.cycle_start(anchor, reference);
        let end = self.budget_period.0.next_date(start);
        DateWindow { start, end }
    }
}

#[derive(Default)]
struct Accumulator {
    budgeted: f64,
    real: f64,
    missing_budget: bool,
    missing_real: bool,
}

impl Accumulator {
    fn add_budgeted(&mut self, amount: f64) {
        self.budgeted += amount;
    }

    fn add_real(&mut self, amount: f64) {
        self.real += amount;
    }

    fn is_incomplete(&self) -> bool {
        self.missing_budget || self.missing_real
    }
}
