use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

pub use crate::domain::ledger::{
    AccountBudget, BudgetScope, BudgetStatus, BudgetSummary, BudgetTotals, BudgetTotalsDelta,
    CategoryBudget, ConversionContext, DateWindow, Simulation, SimulationBudgetImpact,
    SimulationChange, SimulationStatus, SimulationTransactionPatch,
};

#[derive(Debug, Clone)]
pub struct ForecastReport {
    pub scope: BudgetScope,
    pub forecast: ForecastResult,
    pub summary: BudgetSummary,
}
use super::{
    account::Account,
    category::Category,
    recurring::{
        forecast_for_window, materialize_due_instances, rebuild_metadata, snapshot_recurrences,
        ForecastResult, RecurrenceSnapshot,
    },
    time_interval::{TimeInterval, TimeUnit},
    transaction::Transaction,
};
use crate::{
    currency::{ConvertedAmount, CurrencyCode, FormatOptions, LocaleConfig, ValuationPolicy},
    errors::LedgerError,
};

pub const CURRENT_SCHEMA_VERSION: u8 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub id: Uuid,
    pub name: String,
    pub budget_period: BudgetPeriod,
    #[serde(default)]
    pub base_currency: CurrencyCode,
    #[serde(default)]
    pub locale: LocaleConfig,
    #[serde(default, skip_serializing_if = "FormatOptions::is_default")]
    pub format: FormatOptions,
    #[serde(default)]
    pub valuation_policy: ValuationPolicy,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub simulations: Vec<Simulation>,
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
            base_currency: CurrencyCode::default(),
            locale: LocaleConfig::default(),
            format: FormatOptions::default(),
            valuation_policy: ValuationPolicy::default(),
            accounts: Vec::new(),
            categories: Vec::new(),
            transactions: Vec::new(),
            simulations: Vec::new(),
            created_at: now,
            updated_at: now,
            schema_version: CURRENT_SCHEMA_VERSION,
        }
    }

    pub fn base_currency(&self) -> &CurrencyCode {
        &self.base_currency
    }

    pub fn conversion_context(&self, report_date: NaiveDate) -> ConversionContext {
        ConversionContext {
            policy: self.valuation_policy.clone(),
            report_date,
        }
    }

    fn account_currency(&self, id: Uuid) -> Option<String> {
        self.account(id).and_then(|acct| acct.currency.clone())
    }

    pub fn transaction_currency(&self, txn: &Transaction) -> CurrencyCode {
        if let Some(code) = &txn.currency {
            return CurrencyCode::new(code.clone());
        }
        self.account_currency(txn.from_account)
            .or_else(|| self.account_currency(txn.to_account))
            .map(CurrencyCode::new)
            .unwrap_or_else(|| self.base_currency.clone())
    }

    fn convert_amount(
        &self,
        amount: f64,
        from: &CurrencyCode,
        txn_date: NaiveDate,
        ctx: &ConversionContext,
    ) -> Result<ConvertedAmount, LedgerError> {
        let target = self.base_currency();
        if from.as_str() == target.as_str() {
            return Ok(ConvertedAmount {
                amount,
                rate_used: 1.0,
                rate_date: ctx.effective_date(txn_date),
                source: "base currency parity".into(),
                from: from.clone(),
                to: target.clone(),
            });
        }
        Err(LedgerError::InvalidInput(format!(
            "cannot convert from {} to {}: FX rates are disabled",
            from.as_str(),
            target.as_str()
        )))
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
        self.refresh_recurrence_metadata();
        self.touch();
        id
    }

    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    pub fn account(&self, id: Uuid) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn account_mut(&mut self, id: Uuid) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|account| account.id == id)
    }

    pub fn category(&self, id: Uuid) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    pub fn category_mut(&mut self, id: Uuid) -> Option<&mut Category> {
        self.categories
            .iter_mut()
            .find(|category| category.id == id)
    }

    pub fn transaction(&self, id: Uuid) -> Option<&Transaction> {
        self.transactions
            .iter()
            .find(|transaction| transaction.id == id)
    }

    pub fn transaction_mut(&mut self, id: Uuid) -> Option<&mut Transaction> {
        self.transactions
            .iter_mut()
            .find(|transaction| transaction.id == id)
    }

    pub fn remove_transaction(&mut self, id: Uuid) -> Option<Transaction> {
        if let Some(pos) = self
            .transactions
            .iter()
            .position(|transaction| transaction.id == id)
        {
            let removed = self.transactions.remove(pos);
            self.refresh_recurrence_metadata();
            self.touch();
            Some(removed)
        } else {
            None
        }
    }

    pub fn simulations(&self) -> &[Simulation] {
        &self.simulations
    }

    pub fn simulation(&self, name: &str) -> Option<&Simulation> {
        self.simulations
            .iter()
            .find(|sim| sim.name.eq_ignore_ascii_case(name))
    }

    pub fn simulation_mut(&mut self, name: &str) -> Option<&mut Simulation> {
        self.simulations
            .iter_mut()
            .find(|sim| sim.name.eq_ignore_ascii_case(name))
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn schema_version_default() -> u8 {
        CURRENT_SCHEMA_VERSION
    }

    pub fn migrate_from_schema(&mut self, original_version: u8) -> Vec<String> {
        let mut notes = Vec::new();
        if original_version < CURRENT_SCHEMA_VERSION {
            notes.push(format!(
                "upgraded schema from v{} to v{}",
                original_version, CURRENT_SCHEMA_VERSION
            ));
        }
        if original_version < 3 {
            self.refresh_recurrence_metadata();
            notes.push("refreshed recurrence metadata for schema v3".into());
        }
        if original_version < 4 {
            if self.base_currency.as_str().is_empty() {
                self.base_currency = CurrencyCode::default();
            }
            if self.locale.language_tag.is_empty() {
                self.locale = LocaleConfig::default();
            }
            self.format = FormatOptions::default();
            self.valuation_policy = ValuationPolicy::TransactionDate;
            notes.push("initialized currency/localization defaults for schema v4".into());
        }
        self.schema_version = CURRENT_SCHEMA_VERSION;
        notes
    }

    pub fn upgrade_schema_if_needed(&mut self) -> bool {
        if self.schema_version < CURRENT_SCHEMA_VERSION {
            self.schema_version = CURRENT_SCHEMA_VERSION;
            self.touch();
            true
        } else {
            false
        }
    }

    pub fn create_simulation(
        &mut self,
        name: impl Into<String>,
        notes: Option<String>,
    ) -> Result<&Simulation, LedgerError> {
        let name = name.into();
        if self
            .simulations
            .iter()
            .any(|sim| sim.name.eq_ignore_ascii_case(&name))
        {
            return Err(LedgerError::InvalidInput(format!(
                "simulation `{}` already exists",
                name
            )));
        }
        let now = Utc::now();
        let simulation = Simulation {
            name,
            notes,
            status: SimulationStatus::Pending,
            created_at: now,
            updated_at: now,
            applied_at: None,
            changes: Vec::new(),
        };
        self.simulations.push(simulation);
        self.touch();
        Ok(self.simulations.last().unwrap())
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

    pub fn budget_window_for(&self, reference: NaiveDate) -> DateWindow {
        self.budget_window_containing(reference)
    }

    pub fn summarize_window_scope(&self, window: DateWindow, scope: BudgetScope) -> BudgetSummary {
        self.summarize_window(window, scope, None)
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

    pub fn recurrence_snapshots(&self, reference: NaiveDate) -> Vec<RecurrenceSnapshot> {
        snapshot_recurrences(&self.transactions, reference)
    }

    pub fn forecast_window_report(
        &self,
        window: DateWindow,
        reference: NaiveDate,
        simulation: Option<&str>,
    ) -> Result<ForecastReport, LedgerError> {
        let scope = window.scope(reference);
        let base_transactions = if let Some(name) = simulation {
            let sim = self.simulation(name).ok_or_else(|| {
                LedgerError::InvalidRef(format!("simulation `{}` not found", name))
            })?;
            self.transactions_with_simulation(sim)?
        } else {
            self.transactions.clone()
        };
        let forecast = forecast_for_window(window, reference, &base_transactions);
        let mut overlay = base_transactions.clone();
        overlay.extend(
            forecast
                .transactions
                .iter()
                .map(|item| item.transaction.clone()),
        );
        let summary = self.summarize_window(window, scope, Some(&overlay));
        Ok(ForecastReport {
            scope,
            forecast,
            summary,
        })
    }

    pub fn materialize_due_recurrences(&mut self, reference: NaiveDate) -> usize {
        let pending = materialize_due_instances(reference, &self.transactions);
        if pending.is_empty() {
            return 0;
        }
        let created = pending.len();
        self.transactions.extend(pending);
        self.refresh_recurrence_metadata();
        self.touch();
        created
    }

    pub fn refresh_recurrence_metadata(&mut self) {
        if self
            .transactions
            .iter()
            .all(|txn| txn.recurrence.is_none() && txn.recurrence_series_id.is_none())
        {
            return;
        }
        let metadata = rebuild_metadata(&self.transactions);
        if metadata.is_empty() {
            return;
        }
        for txn in &mut self.transactions {
            if let Some(recurrence) = txn.recurrence.as_mut() {
                let series_id = if recurrence.series_id.is_nil() {
                    txn.id
                } else {
                    recurrence.series_id
                };
                if let Some(series) = metadata.get(&series_id) {
                    recurrence.update_metadata(
                        series.last_generated,
                        series.last_completed,
                        series.next_due,
                        series.total_occurrences,
                    );
                }
            }
        }
    }

    pub fn add_simulation_transaction(
        &mut self,
        sim_name: &str,
        mut transaction: Transaction,
    ) -> Result<(), LedgerError> {
        transaction.id = Uuid::new_v4();
        let sim = self.editable_simulation(sim_name)?;
        sim.changes
            .push(SimulationChange::AddTransaction { transaction });
        sim.updated_at = Utc::now();
        self.touch();
        Ok(())
    }

    pub fn exclude_transaction_in_simulation(
        &mut self,
        sim_name: &str,
        transaction_id: Uuid,
    ) -> Result<(), LedgerError> {
        if !self.transactions.iter().any(|t| t.id == transaction_id) {
            return Err(LedgerError::InvalidRef(format!(
                "transaction {} not found",
                transaction_id
            )));
        }
        let sim = self.editable_simulation(sim_name)?;
        sim.changes
            .push(SimulationChange::ExcludeTransaction { transaction_id });
        sim.updated_at = Utc::now();
        self.touch();
        Ok(())
    }

    pub fn modify_transaction_in_simulation(
        &mut self,
        sim_name: &str,
        patch: SimulationTransactionPatch,
    ) -> Result<(), LedgerError> {
        if !self
            .transactions
            .iter()
            .any(|t| t.id == patch.transaction_id)
        {
            return Err(LedgerError::InvalidRef(format!(
                "transaction {} not found",
                patch.transaction_id
            )));
        }
        let sim = self.editable_simulation(sim_name)?;
        sim.changes.push(SimulationChange::ModifyTransaction(patch));
        sim.updated_at = Utc::now();
        self.touch();
        Ok(())
    }

    pub fn apply_simulation(&mut self, sim_name: &str) -> Result<(), LedgerError> {
        let index = self
            .simulations
            .iter()
            .position(|sim| sim.name.eq_ignore_ascii_case(sim_name))
            .ok_or_else(|| {
                LedgerError::InvalidRef(format!("simulation `{}` not found", sim_name))
            })?;
        if self.simulations[index].status != SimulationStatus::Pending {
            return Err(LedgerError::InvalidInput(format!(
                "simulation `{}` is not pending",
                sim_name
            )));
        }

        let changes = self.simulations[index].changes.clone();

        let mut applied = self.transactions.clone();
        for change in &changes {
            match change {
                SimulationChange::AddTransaction { transaction } => {
                    applied.push(transaction.clone());
                }
                SimulationChange::ModifyTransaction(patch) => {
                    let txn = applied
                        .iter_mut()
                        .find(|t| t.id == patch.transaction_id)
                        .ok_or_else(|| {
                            LedgerError::InvalidRef(format!(
                                "transaction {} not found",
                                patch.transaction_id
                            ))
                        })?;
                    apply_patch(txn, patch);
                }
                SimulationChange::ExcludeTransaction { transaction_id } => {
                    let before = applied.len();
                    applied.retain(|t| t.id != *transaction_id);
                    if before == applied.len() {
                        return Err(LedgerError::InvalidRef(format!(
                            "transaction {} not found",
                            transaction_id
                        )));
                    }
                }
            }
        }

        self.transactions = applied;
        self.refresh_recurrence_metadata();
        let simulation = &mut self.simulations[index];
        simulation.status = SimulationStatus::Applied;
        simulation.applied_at = Some(Utc::now());
        simulation.updated_at = Utc::now();
        self.touch();
        Ok(())
    }

    pub fn discard_simulation(&mut self, sim_name: &str) -> Result<(), LedgerError> {
        let len_before = self.simulations.len();
        self.simulations
            .retain(|sim| !sim.name.eq_ignore_ascii_case(sim_name));
        if len_before == self.simulations.len() {
            return Err(LedgerError::InvalidRef(format!(
                "simulation `{}` not found",
                sim_name
            )));
        }
        self.touch();
        Ok(())
    }

    pub fn simulation_changes(&self, sim_name: &str) -> Result<&[SimulationChange], LedgerError> {
        let sim = self.simulation(sim_name).ok_or_else(|| {
            LedgerError::InvalidRef(format!("simulation `{}` not found", sim_name))
        })?;
        Ok(&sim.changes)
    }

    pub fn summarize_simulation_in_window(
        &self,
        simulation_name: &str,
        window: DateWindow,
        scope: BudgetScope,
    ) -> Result<SimulationBudgetImpact, LedgerError> {
        let simulation = self.simulation(simulation_name).ok_or_else(|| {
            LedgerError::InvalidRef(format!("simulation `{}` not found", simulation_name))
        })?;
        if simulation.status == SimulationStatus::Discarded {
            return Err(LedgerError::InvalidInput(format!(
                "simulation `{}` is discarded",
                simulation_name
            )));
        }
        let simulated_transactions = self.transactions_with_simulation(simulation)?;
        let base = self.summarize_window(window, scope, None);
        let simulated = self.summarize_window(window, scope, Some(&simulated_transactions));
        let delta = BudgetTotalsDelta {
            budgeted: simulated.totals.budgeted - base.totals.budgeted,
            real: simulated.totals.real - base.totals.real,
            remaining: simulated.totals.remaining - base.totals.remaining,
            variance: simulated.totals.variance - base.totals.variance,
        };
        Ok(SimulationBudgetImpact {
            simulation_name: simulation.name.clone(),
            base,
            simulated,
            delta,
        })
    }

    pub fn summarize_simulation_current(
        &self,
        simulation_name: &str,
    ) -> Result<SimulationBudgetImpact, LedgerError> {
        let today = Utc::now().date_naive();
        let window = self.budget_window_containing(today);
        let scope = window.scope(today);
        self.summarize_simulation_in_window(simulation_name, window, scope)
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
        let mut warnings = Vec::new();
        let mut disclosures: BTreeSet<String> = BTreeSet::new();

        let report_reference = window
            .end
            .checked_sub_signed(Duration::days(1))
            .unwrap_or(window.end);
        let ctx = self.conversion_context(report_reference);
        disclosures.insert(format!(
            "Valuation policy: {:?} (report date {})",
            self.valuation_policy, report_reference
        ));

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

            if !budget_in && !actual_in {
                continue;
            }

            let mut txn_incomplete = false;
            let cat_entry = category_map.entry(txn.category_id).or_default();
            let account_entry = account_map.entry(txn.from_account).or_default();
            let txn_currency = self.transaction_currency(txn);

            if budget_in {
                match self.convert_amount(
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
                    match self.convert_amount(amount, &txn_currency, actual_date, &ctx) {
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

    fn transactions_with_simulation(
        &self,
        simulation: &Simulation,
    ) -> Result<Vec<Transaction>, LedgerError> {
        let mut snapshot = self.transactions.clone();
        for change in &simulation.changes {
            match change {
                SimulationChange::AddTransaction { transaction } => {
                    snapshot.push(transaction.clone());
                }
                SimulationChange::ModifyTransaction(patch) => {
                    let txn = snapshot
                        .iter_mut()
                        .find(|t| t.id == patch.transaction_id)
                        .ok_or_else(|| {
                            LedgerError::InvalidRef(format!(
                                "transaction {} not found for simulation `{}`",
                                patch.transaction_id, simulation.name
                            ))
                        })?;
                    apply_patch(txn, patch);
                }
                SimulationChange::ExcludeTransaction { transaction_id } => {
                    let before = snapshot.len();
                    snapshot.retain(|t| t.id != *transaction_id);
                    if before == snapshot.len() {
                        return Err(LedgerError::InvalidRef(format!(
                            "transaction {} not found for simulation `{}`",
                            transaction_id, simulation.name
                        )));
                    }
                }
            }
        }
        Ok(snapshot)
    }

    fn editable_simulation(&mut self, name: &str) -> Result<&mut Simulation, LedgerError> {
        let sim = self
            .simulation_mut(name)
            .ok_or_else(|| LedgerError::InvalidRef(format!("simulation `{}` not found", name)))?;
        if sim.status != SimulationStatus::Pending {
            return Err(LedgerError::InvalidInput(format!(
                "simulation `{}` is not editable",
                name
            )));
        }
        Ok(sim)
    }
}

fn apply_patch(txn: &mut Transaction, patch: &SimulationTransactionPatch) {
    if let Some(value) = patch.from_account {
        txn.from_account = value;
    }
    if let Some(value) = patch.to_account {
        txn.to_account = value;
    }
    if let Some(category) = &patch.category_id {
        txn.category_id = *category;
    }
    if let Some(date) = patch.scheduled_date {
        txn.scheduled_date = date;
    }
    if let Some(actual_date) = &patch.actual_date {
        txn.actual_date = *actual_date;
    }
    if let Some(amount) = patch.budgeted_amount {
        txn.budgeted_amount = amount;
    }
    if let Some(actual_amount) = &patch.actual_amount {
        txn.actual_amount = *actual_amount;
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
