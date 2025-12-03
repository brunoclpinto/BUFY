//! Provides budget aggregation and comparison helpers across ledger data.

use std::collections::{BTreeSet, HashMap};

use chrono::{Duration, NaiveDate, Utc};
use uuid::Uuid;

use crate::currency::ConvertedAmount;
use bufy_domain::ledger::{
    AccountBudget, BudgetScope, BudgetSummary, BudgetTotals, CategoryBudget,
    CategoryBudgetAssignment, CategoryBudgetStatus, CategoryBudgetSummary,
    CategoryBudgetSummaryKind, DateWindow,
};
use crate::ledger::{account::Account, category::Category, transaction::Transaction, Ledger};

/// Stateless budgeting utilities that operate over [`Ledger`] snapshots.
pub struct BudgetService;

impl BudgetService {
    /// Summarizes the ledger's current budget period.
    pub fn summarize_current_period(ledger: &Ledger) -> BudgetSummary {
        let today = Utc::now().date_naive();
        Self::summarize_period_containing(ledger, today)
    }

    /// Summarizes the budget period that contains the given reference date.
    pub fn summarize_period_containing(ledger: &Ledger, date: NaiveDate) -> BudgetSummary {
        let window = ledger.budget_window_for(date);
        let scope = window.scope(date);
        Self::summarize_window_scope(ledger, window, scope)
    }

    /// Summarizes the supplied window and scope using the ledger transactions.
    pub fn summarize_window_scope(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
    ) -> BudgetSummary {
        Self::summarize_window_internal(ledger, window, scope, None)
    }

    /// Summarizes the supplied window and scope against an override list of transactions.
    pub fn summarize_window_with_transactions(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
        transactions: &[Transaction],
    ) -> BudgetSummary {
        Self::summarize_window_internal(ledger, window, scope, Some(transactions))
    }

    /// Returns the totals for a specific category within the provided window.
    pub fn category_totals_in_window(
        ledger: &Ledger,
        category_id: Uuid,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Option<BudgetTotals> {
        let summary = Self::summarize_window_scope(ledger, window, scope);
        summary
            .per_category
            .into_iter()
            .find(|entry| entry.category_id == Some(category_id))
            .map(|entry| entry.totals)
    }

    /// Returns the budget status for a single category, combining assigned budget data with totals.
    pub fn category_budget_status(
        ledger: &Ledger,
        category_id: Uuid,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Option<CategoryBudgetStatus> {
        let totals = Self::category_totals_in_window(ledger, category_id, window, scope)?;
        ledger
            .category(category_id)
            .map(|category| CategoryBudgetStatus {
                category_id,
                name: category.name.clone(),
                budget: category.budget.clone(),
                totals,
            })
    }

    /// Lists all categories and their budget usage for a window.
    pub fn category_budget_statuses(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Vec<CategoryBudgetStatus> {
        let summary = Self::summarize_window_scope(ledger, window, scope);
        let totals_by_category: HashMap<Uuid, BudgetTotals> = summary
            .per_category
            .into_iter()
            .filter_map(|entry| entry.category_id.map(|id| (id, entry.totals)))
            .collect();
        ledger
            .categories
            .iter()
            .map(|category| CategoryBudgetStatus {
                category_id: category.id,
                name: category.name.clone(),
                budget: category.budget.clone(),
                totals: totals_by_category
                    .get(&category.id)
                    .cloned()
                    .unwrap_or_else(|| BudgetTotals::from_parts(0.0, 0.0, false)),
            })
            .collect()
    }

    /// Lists every category with an assigned budget definition.
    pub fn categories_with_budgets(ledger: &Ledger) -> Vec<CategoryBudgetAssignment> {
        ledger
            .categories
            .iter()
            .filter_map(|category| {
                category
                    .budget
                    .as_ref()
                    .map(|budget| CategoryBudgetAssignment {
                        category_id: category.id,
                        name: category.name.clone(),
                        budget: budget.clone(),
                    })
            })
            .collect()
    }

    /// Builds detailed summaries for categories with budgets using canonical ledger totals.
    pub fn category_budget_summaries(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
        kind: CategoryBudgetSummaryKind,
    ) -> Vec<CategoryBudgetSummary> {
        Self::category_budget_summaries_with_transactions(ledger, window, scope, None, kind)
    }

    /// Builds category budget summaries using an alternate set of transactions.
    pub fn category_budget_summaries_with_transactions(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
        tx_override: Option<&[Transaction]>,
        kind: CategoryBudgetSummaryKind,
    ) -> Vec<CategoryBudgetSummary> {
        let summary = Self::summarize_window_internal(ledger, window, scope, tx_override);
        let totals_by_category: HashMap<Uuid, BudgetTotals> = summary
            .per_category
            .into_iter()
            .filter_map(|entry| entry.category_id.map(|id| (id, entry.totals)))
            .collect();
        ledger
            .categories
            .iter()
            .filter_map(|category| {
                let budget = category.budget.as_ref()?;
                let spent = totals_by_category
                    .get(&category.id)
                    .map(|totals| totals.real)
                    .unwrap_or(0.0);
                Some(CategoryBudgetSummary::from_definition(
                    category.id,
                    category.name.clone(),
                    budget,
                    spent,
                    kind.clone(),
                ))
            })
            .collect()
    }

    pub(crate) fn summarize_window_internal(
        ledger: &Ledger,
        window: DateWindow,
        scope: BudgetScope,
        tx_override: Option<&[Transaction]>,
    ) -> BudgetSummary {
        let txs = tx_override.unwrap_or(&ledger.transactions);
        let mut totals_acc = Accumulator::default();
        let mut category_map: HashMap<Option<Uuid>, Accumulator> = HashMap::new();
        let mut account_map: HashMap<Uuid, Accumulator> = HashMap::new();
        let mut orphaned = 0usize;
        let mut incomplete_transactions = 0usize;
        let mut warnings = Vec::new();
        let mut disclosures: BTreeSet<String> = BTreeSet::new();

        let report_reference = window
            .end
            .checked_sub_signed(Duration::days(1))
            .unwrap_or(window.end);
        let ctx = ledger.conversion_context(report_reference);
        disclosures.insert(format!(
            "Valuation policy: {:?} (report date {})",
            ledger.valuation_policy, report_reference
        ));

        let category_lookup: HashMap<Uuid, &Category> =
            ledger.categories.iter().map(|c| (c.id, c)).collect();
        let account_lookup: HashMap<Uuid, &Account> =
            ledger.accounts.iter().map(|a| (a.id, a)).collect();

        for txn in txs {
            let budget_in = window.contains(txn.scheduled_date);
            let actual_in = txn
                .actual_date
                .map(|date| window.contains(date))
                .unwrap_or(false);
            let actual_amount = txn.actual_amount;

            if !budget_in && !actual_in {
                continue;
            }

            let mut txn_incomplete = false;
            let cat_entry = category_map.entry(txn.category_id).or_default();
            let account_entry = account_map.entry(txn.from_account).or_default();
            let txn_currency = ledger.transaction_currency(txn);

            if budget_in {
                match ledger.convert_amount(
                    txn.budgeted_amount,
                    &txn_currency,
                    txn.scheduled_date,
                    &ctx,
                ) {
                    Ok(converted) => {
                        record_disclosure(&mut disclosures, &converted);
                        totals_acc.add_budgeted(converted.amount);
                        cat_entry.add_budgeted(converted.amount);
                        account_entry.add_budgeted(converted.amount);
                    }
                    Err(err) => {
                        warnings.push(format!("{} budget conversion failed: {}", txn.id, err));
                        totals_acc.missing_budget = true;
                        cat_entry.missing_budget = true;
                        account_entry.missing_budget = true;
                        txn_incomplete = true;
                    }
                }
            }

            if actual_in {
                if let Some(amount) = actual_amount {
                    let actual_date = txn.actual_date.unwrap_or(txn.scheduled_date);
                    match ledger.convert_amount(amount, &txn_currency, actual_date, &ctx) {
                        Ok(converted) => {
                            record_disclosure(&mut disclosures, &converted);
                            totals_acc.add_real(converted.amount);
                            cat_entry.add_real(converted.amount);
                            account_entry.add_real(converted.amount);
                        }
                        Err(err) => {
                            warnings.push(format!("{} actual conversion failed: {}", txn.id, err));
                            totals_acc.missing_real = true;
                            cat_entry.missing_real = true;
                            account_entry.missing_real = true;
                            txn_incomplete = true;
                        }
                    }
                } else {
                    totals_acc.missing_real = true;
                    cat_entry.missing_real = true;
                    account_entry.missing_real = true;
                    txn_incomplete = true;
                }
            }

            if actual_in && !budget_in {
                totals_acc.missing_budget = true;
                cat_entry.missing_budget = true;
                account_entry.missing_budget = true;
                txn_incomplete = true;
            }
            if budget_in && txn.actual_amount.is_none() {
                totals_acc.missing_real = true;
                cat_entry.missing_real = true;
                account_entry.missing_real = true;
                txn_incomplete = true;
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

        let mut disclosures_vec: Vec<String> = disclosures.into_iter().collect();
        disclosures_vec.extend(warnings);

        BudgetSummary {
            scope,
            window,
            totals,
            per_category,
            per_account,
            orphaned_transactions: orphaned,
            incomplete_transactions,
            disclosures: disclosures_vec,
        }
    }
}


fn record_disclosure(disclosures: &mut BTreeSet<String>, converted: &ConvertedAmount) {
    disclosures.insert(format!(
        "{} → {} @ {:.6} on {} ({})",
        converted.from.as_str(),
        converted.to.as_str(),
        converted.rate_used,
        converted.rate_date,
        converted.source
    ));
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
