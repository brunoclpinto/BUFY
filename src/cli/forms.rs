//! Wizard-style form framework used by interactive CLI commands.
//!
//! Phase 14 defines a reusable contract for multi-step data entry. Concrete
//! entities (accounts, categories, transactions, …) will plug into this
//! framework in subsequent phases by describing their fields and leveraging the
//! generic form engine implemented here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

use crate::cli::io;
use crate::cli::ui::formatting::Formatter;
use crate::cli::ui::prompts::{
    choice_menu, confirm_menu, text_input, ChoicePromptResult, ConfirmationPromptResult,
    TextPromptResult,
};
use crate::cli::ui::style::{format_header, style};
use crate::cli::ui::table_renderer::visible_width;
use crate::ledger::{
    AccountKind, CategoryKind, Recurrence, RecurrenceMode, TimeInterval, TimeUnit,
    TransactionStatus,
};

/// High-level lifecycle states emitted by the form runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormResult<T> {
    Completed(T),
    Cancelled,
}

/// Describes how prompts can be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptResponse {
    /// User supplied a concrete value.
    Value(String),
    /// User chose to keep the default/current value.
    Keep,
    /// Abort the entire wizard immediately.
    Cancel,
    /// Go back to the previous field.
    Back,
    /// Request additional information for the current field.
    Help,
}

/// Responses accepted when confirming the collected data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationResponse {
    Confirm,
    Back,
    Cancel,
}

/// Field-level validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Supported data kinds for form fields.
#[derive(Debug, Clone)]
pub enum FieldKind {
    Text,
    Integer,
    Decimal,
    Date,
    Time,
    Boolean,
    Choice(Vec<String>),
}

type ValidatorCallback = dyn Fn(&str) -> Result<String, String> + Send + Sync;
type SharedValidatorCallback = Arc<ValidatorCallback>;

/// Built-in validation helpers.
#[derive(Clone)]
pub enum Validator {
    None,
    NonEmpty,
    Integer,
    PositiveNumber,
    Decimal,
    Date,
    Time,
    OneOf(Vec<String>),
    Custom(SharedValidatorCallback),
}

impl Validator {
    fn validate(&self, input: &str) -> Result<String, ValidationError> {
        match self {
            Validator::None => Ok(input.to_string()),
            Validator::NonEmpty => {
                if input.trim().is_empty() {
                    Err(ValidationError::new("Value cannot be empty"))
                } else {
                    Ok(input.trim().to_string())
                }
            }
            Validator::Integer => input
                .trim()
                .parse::<i64>()
                .map(|v| v.to_string())
                .map_err(|_| ValidationError::new("Enter a whole number (e.g., 42)")),
            Validator::PositiveNumber => input
                .trim()
                .parse::<f64>()
                .map_err(|_| ValidationError::new("Enter a numeric value"))
                .and_then(|v| {
                    if v > 0.0 {
                        Ok(v.to_string())
                    } else {
                        Err(ValidationError::new("Value must be greater than zero"))
                    }
                }),
            Validator::Decimal => input
                .trim()
                .parse::<f64>()
                .map(|v| v.to_string())
                .map_err(|_| ValidationError::new("Enter a numeric value")),
            Validator::Date => NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d")
                .map(|d| d.to_string())
                .map_err(|_| ValidationError::new("Use YYYY-MM-DD format")),
            Validator::Time => NaiveTime::parse_from_str(input.trim(), "%H:%M")
                .map(|t| t.format("%H:%M").to_string())
                .map_err(|_| ValidationError::new("Use 24-hour HH:MM format")),
            Validator::OneOf(options) => {
                let normalized = input.trim().to_lowercase();
                options
                    .iter()
                    .find(|candidate| candidate.to_lowercase() == normalized)
                    .cloned()
                    .ok_or_else(|| {
                        ValidationError::new(format!(
                            "Value must be one of: {}",
                            options.join(", ")
                        ))
                    })
            }
            Validator::Custom(func) => func(input).map_err(ValidationError::new),
        }
    }
}

/// Declarative description of a single form field.
#[derive(Clone)]
pub struct FieldDescriptor {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub required: bool,
    pub help: Option<&'static str>,
    pub validator: Validator,
}

impl FieldDescriptor {
    pub fn new(
        key: &'static str,
        label: &'static str,
        kind: FieldKind,
        validator: Validator,
    ) -> Self {
        Self {
            key,
            label,
            kind,
            required: true,
            help: None,
            validator,
        }
    }

    pub fn with_optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn with_help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }
}

/// Metadata describing a full wizard, including field order.
pub struct FormDescriptor {
    pub name: &'static str,
    pub fields: Vec<FieldDescriptor>,
}

impl FormDescriptor {
    pub fn new(name: &'static str, fields: Vec<FieldDescriptor>) -> Self {
        Self { name, fields }
    }
}

/// Utility to present menu-style choices while allowing the user to enter
/// either the display label or its numeric index.
#[derive(Clone)]
struct ChoiceMapper<T: Clone + PartialEq + Send + Sync> {
    display: Vec<String>,
    values: Vec<T>,
    alias_to_index: HashMap<String, usize>,
}

impl<T: Clone + PartialEq + Send + Sync> ChoiceMapper<T> {
    fn from_pairs(pairs: Vec<(String, T)>) -> Self {
        let mut display = Vec::new();
        let mut values = Vec::new();
        let mut alias_to_index = HashMap::new();

        for (idx, (label, value)) in pairs.into_iter().enumerate() {
            let index = idx + 1;
            let display_label = format!("[{}] {}", index, label);
            alias_to_index.insert(index.to_string(), idx);
            alias_to_index.insert(label.to_ascii_lowercase(), idx);
            alias_to_index.insert(display_label.to_ascii_lowercase(), idx);
            if label.eq_ignore_ascii_case("none") {
                alias_to_index.insert("none".into(), idx);
            }
            display.push(display_label);
            values.push(value);
        }

        Self {
            display,
            values,
            alias_to_index,
        }
    }

    fn options(&self) -> Vec<String> {
        self.display.clone()
    }

    fn resolve(&self, input: &str) -> Option<String> {
        let key = input.trim().to_ascii_lowercase();
        self.alias_to_index
            .get(&key)
            .map(|index| self.display[*index].clone())
    }

    fn value_for_display(&self, display: &str) -> Option<&T> {
        self.display
            .iter()
            .position(|candidate| candidate == display)
            .and_then(|index| self.values.get(index))
    }

    fn display_for_value(&self, value: &T) -> Option<String> {
        self.values
            .iter()
            .position(|candidate| candidate == value)
            .map(|index| self.display[index].clone())
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn make_name_validator(existing: HashSet<String>) -> Validator {
    Validator::Custom(Arc::new(move |input| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("Name is required".into());
        }
        let normalized = normalize_name(trimmed);
        if existing.contains(&normalized) {
            Err(format!("Name already exists: `{}`", trimmed))
        } else {
            Ok(trimmed.to_string())
        }
    }))
}

fn make_optional_decimal_validator() -> Validator {
    Validator::Custom(Arc::new(|input| {
        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            Ok(String::new())
        } else {
            trimmed
                .parse::<f64>()
                .map(|value| value.to_string())
                .map_err(|_| "Enter a numeric amount".into())
        }
    }))
}

fn make_non_negative_decimal_validator() -> Validator {
    Validator::Custom(Arc::new(|input| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("Amount is required".into());
        }
        trimmed
            .parse::<f64>()
            .map_err(|_| "Enter a numeric amount".into())
            .and_then(|value| {
                if value < 0.0 {
                    Err("Amount must be zero or positive".into())
                } else {
                    Ok(format_amount(value))
                }
            })
    }))
}

fn make_optional_non_negative_decimal_validator(default: Option<f64>) -> Validator {
    Validator::Custom(Arc::new(move |input| {
        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            if let Some(def) = default {
                Ok(format_amount(def))
            } else {
                Ok(String::new())
            }
        } else {
            trimmed
                .parse::<f64>()
                .map_err(|_| "Enter a numeric amount".into())
                .and_then(|value| {
                    if value < 0.0 {
                        Err("Amount must be zero or positive".into())
                    } else {
                        Ok(format_amount(value))
                    }
                })
        }
    }))
}

fn make_min_date_validator(min_date: NaiveDate) -> Validator {
    Validator::Custom(Arc::new(move |input| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("Date is required (use YYYY-MM-DD)".into());
        }
        NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
            .map_err(|_| "Use YYYY-MM-DD format".to_string())
            .and_then(|date| {
                if date < min_date {
                    Err(format!(
                        "Date must be on or after {}",
                        min_date.format("%Y-%m-%d")
                    ))
                } else {
                    Ok(date.to_string())
                }
            })
    }))
}

fn make_optional_date_validator(max_date: NaiveDate) -> Validator {
    Validator::Custom(Arc::new(move |input| {
        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            Ok(String::new())
        } else {
            NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
                .map_err(|_| "Use YYYY-MM-DD format".to_string())
                .and_then(|date| {
                    if date > max_date {
                        Err(format!(
                            "Date cannot be after {}",
                            max_date.format("%Y-%m-%d")
                        ))
                    } else {
                        Ok(date.to_string())
                    }
                })
        }
    }))
}

fn make_positive_integer_validator(default: u32) -> Validator {
    Validator::Custom(Arc::new(move |input| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default.to_string());
        }
        trimmed
            .parse::<u32>()
            .map_err(|_| "Enter a whole number (1 or greater)".into())
            .and_then(|value| {
                if value == 0 {
                    Err("Value must be at least 1".into())
                } else {
                    Ok(value.to_string())
                }
            })
    }))
}

fn make_notes_validator(max_len: usize) -> Validator {
    Validator::Custom(Arc::new(move |input| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            Ok(String::new())
        } else if trimmed.len() > max_len {
            Err(format!(
                "Notes cannot exceed {} characters (got {})",
                max_len,
                trimmed.len()
            ))
        } else {
            Ok(trimmed.to_string())
        }
    }))
}

fn make_choice_validator<T: Clone + PartialEq + Send + Sync + 'static>(
    mapper: ChoiceMapper<T>,
    field_label: &'static str,
) -> Validator {
    let options = mapper.options();
    let lookup = mapper.clone();
    Validator::Custom(Arc::new(move |input| {
        if let Some(display) = lookup.resolve(input) {
            Ok(display)
        } else {
            Err(format!(
                "Select a valid {} (options: {})",
                field_label,
                options.join(", ")
            ))
        }
    }))
}

fn parse_optional_f64(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<f64>().ok()
    }
}

fn format_amount(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    }
}

fn sanitize_notes(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccountFormData {
    pub id: Option<Uuid>,
    pub name: String,
    pub kind: AccountKind,
    pub category_id: Option<Uuid>,
    pub opening_balance: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AccountInitialData {
    pub id: Uuid,
    pub name: String,
    pub kind: AccountKind,
    pub category_id: Option<Uuid>,
    pub opening_balance: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug)]
enum AccountWizardMode {
    Create,
    Edit { id: Uuid },
}

pub struct AccountWizard {
    descriptor: FormDescriptor,
    defaults: BTreeMap<String, String>,
    mode: AccountWizardMode,
    kind_choices: ChoiceMapper<AccountKind>,
    category_choices: ChoiceMapper<Option<Uuid>>,
}

impl AccountWizard {
    pub fn new_create(
        existing_names: HashSet<String>,
        categories: Vec<(String, Option<Uuid>)>,
    ) -> Self {
        Self::build(existing_names, None, categories)
    }

    pub fn new_edit(
        existing_names: HashSet<String>,
        initial: AccountInitialData,
        categories: Vec<(String, Option<Uuid>)>,
    ) -> Self {
        Self::build(existing_names, Some(initial), categories)
    }

    fn build(
        existing_names: HashSet<String>,
        initial: Option<AccountInitialData>,
        categories: Vec<(String, Option<Uuid>)>,
    ) -> Self {
        let mut name_set: HashSet<String> = existing_names
            .into_iter()
            .map(|value| value.to_ascii_lowercase())
            .collect();

        if let Some(data) = &initial {
            name_set.remove(&normalize_name(&data.name));
        }

        let name_validator = make_name_validator(name_set);

        let kind_pairs = vec![
            ("Bank".to_string(), AccountKind::Bank),
            ("Cash".to_string(), AccountKind::Cash),
            ("Savings".to_string(), AccountKind::Savings),
            (
                "Expense destination".to_string(),
                AccountKind::ExpenseDestination,
            ),
            ("Income source".to_string(), AccountKind::IncomeSource),
            ("Unknown".to_string(), AccountKind::Unknown),
        ];
        let kind_choices = ChoiceMapper::from_pairs(kind_pairs);
        let kind_validator = make_choice_validator(kind_choices.clone(), "account type");

        let mut category_pairs = Vec::new();
        category_pairs.push(("None".to_string(), None));
        for (label, id) in categories {
            category_pairs.push((label, id));
        }
        let category_choices = ChoiceMapper::from_pairs(category_pairs);
        let category_validator = make_choice_validator(category_choices.clone(), "linked category");

        let fields = vec![
            FieldDescriptor::new("name", "Account name", FieldKind::Text, name_validator),
            FieldDescriptor::new(
                "kind",
                "Account type",
                FieldKind::Choice(kind_choices.options()),
                kind_validator,
            ),
            FieldDescriptor::new(
                "category",
                "Linked category",
                FieldKind::Choice(category_choices.options()),
                category_validator,
            )
            .with_optional(),
            FieldDescriptor::new(
                "opening_balance",
                "Opening balance",
                FieldKind::Decimal,
                make_optional_decimal_validator(),
            )
            .with_optional(),
            FieldDescriptor::new("notes", "Notes", FieldKind::Text, make_notes_validator(512))
                .with_optional(),
        ];

        let mut defaults = BTreeMap::new();
        let mode = if let Some(data) = initial {
            defaults.insert("name".into(), data.name.clone());
            if let Some(display) = kind_choices.display_for_value(&data.kind) {
                defaults.insert("kind".into(), display);
            }
            if let Some(display) = category_choices.display_for_value(&data.category_id) {
                defaults.insert("category".into(), display);
            }
            if let Some(balance) = data.opening_balance {
                defaults.insert("opening_balance".into(), format_amount(balance));
            }
            if let Some(notes) = data.notes {
                defaults.insert("notes".into(), notes);
            }
            AccountWizardMode::Edit { id: data.id }
        } else {
            if let Some(display) = kind_choices.display_for_value(&AccountKind::Bank) {
                defaults.insert("kind".into(), display);
            }
            if let Some(display) = category_choices.display_for_value(&None) {
                defaults.insert("category".into(), display);
            }
            AccountWizardMode::Create
        };

        Self {
            descriptor: FormDescriptor::new("account", fields),
            defaults,
            mode,
            kind_choices,
            category_choices,
        }
    }
}

impl FormFlow for AccountWizard {
    type Output = AccountFormData;
    type Error = Infallible;

    fn descriptor(&self) -> &FormDescriptor {
        &self.descriptor
    }

    fn defaults(&self) -> BTreeMap<String, String> {
        self.defaults.clone()
    }

    fn commit(&self, values: BTreeMap<String, String>) -> Result<Self::Output, Self::Error> {
        let name = values.get("name").cloned().unwrap_or_default();

        let kind_display = values
            .get("kind")
            .cloned()
            .or_else(|| self.defaults.get("kind").cloned())
            .or_else(|| self.kind_choices.display_for_value(&AccountKind::Bank))
            .unwrap_or_else(|| "Bank".into());
        let kind = self
            .kind_choices
            .value_for_display(&kind_display)
            .cloned()
            .unwrap_or(AccountKind::Bank);

        let category_display = values
            .get("category")
            .cloned()
            .or_else(|| self.defaults.get("category").cloned())
            .or_else(|| self.category_choices.display_for_value(&None))
            .unwrap_or_else(|| "None".into());
        let category_id = self
            .category_choices
            .value_for_display(&category_display)
            .cloned()
            .unwrap_or(None);

        let opening_balance = values
            .get("opening_balance")
            .and_then(|val| parse_optional_f64(val));
        let notes = values.get("notes").and_then(|val| sanitize_notes(val));

        let id = match self.mode {
            AccountWizardMode::Create => None,
            AccountWizardMode::Edit { id } => Some(id),
        };

        Ok(AccountFormData {
            id,
            name,
            kind,
            category_id,
            opening_balance,
            notes,
        })
    }

    fn cancel(&self) -> Self::Error {
        unreachable!()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransactionRecurrenceAction {
    Clear,
    Set(Recurrence),
    Keep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionFormData {
    pub id: Option<Uuid>,
    pub from_account: Uuid,
    pub to_account: Uuid,
    pub category_id: Option<Uuid>,
    pub scheduled_date: NaiveDate,
    pub actual_date: Option<NaiveDate>,
    pub budgeted_amount: f64,
    pub actual_amount: Option<f64>,
    pub status: TransactionStatus,
    pub notes: Option<String>,
    pub recurrence: TransactionRecurrenceAction,
}

#[derive(Clone, Debug)]
pub struct TransactionInitialData {
    pub id: Uuid,
    pub from_account: Uuid,
    pub to_account: Uuid,
    pub category_id: Option<Uuid>,
    pub scheduled_date: NaiveDate,
    pub actual_date: Option<NaiveDate>,
    pub budgeted_amount: f64,
    pub actual_amount: Option<f64>,
    pub recurrence: Option<Recurrence>,
    pub status: TransactionStatus,
    pub notes: Option<String>,
}

#[derive(Clone)]
enum TransactionWizardMode {
    Create {
        default_status: TransactionStatus,
    },
    Edit {
        initial: Box<TransactionInitialData>,
    },
}

#[derive(Clone, PartialEq)]
enum RecurrenceChoice {
    None,
    Preset(TimeInterval),
    EveryNDays,
    KeepExisting,
}

pub struct TransactionWizard {
    descriptor: FormDescriptor,
    defaults: BTreeMap<String, String>,
    mode: TransactionWizardMode,
    account_choices: ChoiceMapper<Uuid>,
    category_choices: ChoiceMapper<Option<Uuid>>,
    recurrence_choices: ChoiceMapper<RecurrenceChoice>,
    status_choices: ChoiceMapper<TransactionStatus>,
}

impl TransactionWizard {
    pub fn new_create(
        accounts: Vec<(String, Uuid)>,
        categories: Vec<(String, Option<Uuid>)>,
        today: NaiveDate,
        min_date: NaiveDate,
        default_status: TransactionStatus,
    ) -> Self {
        Self::build(
            accounts,
            categories,
            today,
            min_date,
            TransactionWizardMode::Create { default_status },
        )
    }

    pub fn new_edit(
        accounts: Vec<(String, Uuid)>,
        categories: Vec<(String, Option<Uuid>)>,
        today: NaiveDate,
        min_date: NaiveDate,
        initial: TransactionInitialData,
    ) -> Self {
        Self::build(
            accounts,
            categories,
            today,
            min_date,
            TransactionWizardMode::Edit {
                initial: Box::new(initial),
            },
        )
    }

    fn build(
        accounts: Vec<(String, Uuid)>,
        categories: Vec<(String, Option<Uuid>)>,
        today: NaiveDate,
        min_date: NaiveDate,
        mode: TransactionWizardMode,
    ) -> Self {
        let account_choices = ChoiceMapper::from_pairs(accounts);
        let account_validator = make_choice_validator(account_choices.clone(), "account");

        let mut category_pairs = Vec::new();
        category_pairs.push(("None".to_string(), None));
        for (label, id) in categories {
            category_pairs.push((label, id));
        }
        let category_choices = ChoiceMapper::from_pairs(category_pairs);
        let category_validator =
            make_choice_validator(category_choices.clone(), "category selection");

        let mut recurrence_pairs = vec![
            ("None".to_string(), RecurrenceChoice::None),
            (
                "Daily".to_string(),
                RecurrenceChoice::Preset(TimeInterval {
                    every: 1,
                    unit: TimeUnit::Day,
                }),
            ),
            (
                "Weekly".to_string(),
                RecurrenceChoice::Preset(TimeInterval {
                    every: 1,
                    unit: TimeUnit::Week,
                }),
            ),
            (
                "Monthly".to_string(),
                RecurrenceChoice::Preset(TimeInterval {
                    every: 1,
                    unit: TimeUnit::Month,
                }),
            ),
            ("Every N days".to_string(), RecurrenceChoice::EveryNDays),
            (
                "Yearly".to_string(),
                RecurrenceChoice::Preset(TimeInterval {
                    every: 1,
                    unit: TimeUnit::Year,
                }),
            ),
        ];

        let defaults = BTreeMap::new();
        let mut recurrence_default_value = RecurrenceChoice::None;
        let mut recurrence_every_default = 1u32;
        let mut keep_display: Option<String> = None;

        let default_status = match &mode {
            TransactionWizardMode::Create { default_status } => default_status.clone(),
            TransactionWizardMode::Edit { initial } => initial.status.clone(),
        };

        if let TransactionWizardMode::Edit { initial } = &mode {
            if let Some(rule) = &initial.recurrence {
                match (&rule.interval.unit, rule.interval.every) {
                    (TimeUnit::Day, 1) => {
                        recurrence_default_value = RecurrenceChoice::Preset(TimeInterval {
                            every: 1,
                            unit: TimeUnit::Day,
                        });
                    }
                    (TimeUnit::Week, 1) => {
                        recurrence_default_value = RecurrenceChoice::Preset(TimeInterval {
                            every: 1,
                            unit: TimeUnit::Week,
                        });
                    }
                    (TimeUnit::Month, 1) => {
                        recurrence_default_value = RecurrenceChoice::Preset(TimeInterval {
                            every: 1,
                            unit: TimeUnit::Month,
                        });
                    }
                    (TimeUnit::Year, 1) => {
                        recurrence_default_value = RecurrenceChoice::Preset(TimeInterval {
                            every: 1,
                            unit: TimeUnit::Year,
                        });
                    }
                    (TimeUnit::Day, every) => {
                        recurrence_default_value = RecurrenceChoice::EveryNDays;
                        recurrence_every_default = every;
                    }
                    _ => {
                        keep_display = Some(format!("Keep existing ({})", rule.interval.label()));
                        recurrence_default_value = RecurrenceChoice::KeepExisting;
                    }
                }
            }
        }

        if let Some(label) = &keep_display {
            recurrence_pairs.push((label.clone(), RecurrenceChoice::KeepExisting));
        }

        let recurrence_choices = ChoiceMapper::from_pairs(recurrence_pairs);
        let recurrence_validator =
            make_choice_validator(recurrence_choices.clone(), "recurrence pattern");

        let status_choices = ChoiceMapper::from_pairs(vec![
            ("Planned".to_string(), TransactionStatus::Planned),
            ("Completed".to_string(), TransactionStatus::Completed),
            ("Missed".to_string(), TransactionStatus::Missed),
            ("Simulated".to_string(), TransactionStatus::Simulated),
        ]);
        let status_validator = make_choice_validator(status_choices.clone(), "transaction status");

        let account_validator_for_to = account_validator.clone();
        let mut fields = vec![
            FieldDescriptor::new(
                "from_account",
                "From account",
                FieldKind::Choice(account_choices.options()),
                account_validator,
            ),
            FieldDescriptor::new(
                "to_account",
                "To account",
                FieldKind::Choice(account_choices.options()),
                account_validator_for_to,
            ),
            FieldDescriptor::new(
                "category",
                "Category",
                FieldKind::Choice(category_choices.options()),
                category_validator,
            )
            .with_optional(),
            FieldDescriptor::new(
                "scheduled_date",
                "Scheduled date (YYYY-MM-DD)",
                FieldKind::Date,
                make_min_date_validator(min_date),
            ),
            FieldDescriptor::new(
                "actual_date",
                "Actual date (YYYY-MM-DD)",
                FieldKind::Date,
                make_optional_date_validator(today),
            )
            .with_optional(),
            FieldDescriptor::new(
                "budgeted_amount",
                "Budgeted amount",
                FieldKind::Decimal,
                make_non_negative_decimal_validator(),
            ),
            FieldDescriptor::new(
                "actual_amount",
                "Actual amount",
                FieldKind::Decimal,
                make_optional_non_negative_decimal_validator(None),
            )
            .with_optional(),
            FieldDescriptor::new(
                "recurrence",
                "Recurrence",
                FieldKind::Choice(recurrence_choices.options()),
                recurrence_validator,
            ),
            FieldDescriptor::new(
                "recurrence_days",
                "Every N days interval",
                FieldKind::Integer,
                make_positive_integer_validator(1),
            )
            .with_help("Only used when recurrence is set to 'Every N days'."),
            FieldDescriptor::new(
                "status",
                "Status",
                FieldKind::Choice(status_choices.options()),
                status_validator,
            ),
            FieldDescriptor::new("notes", "Notes", FieldKind::Text, make_notes_validator(512))
                .with_optional(),
        ];

        let mut defaults = defaults;
        if let Some(display) = account_choices.options().first() {
            defaults.insert("from_account".into(), display.clone());
            defaults.insert("to_account".into(), display.clone());
        }
        if let Some(display) = category_choices.display_for_value(&None) {
            defaults.insert("category".into(), display);
        }
        defaults.insert("scheduled_date".into(), today.to_string());
        defaults.insert(
            "recurrence_days".into(),
            recurrence_every_default.to_string(),
        );

        let recurrence_default_display = recurrence_choices
            .display_for_value(&recurrence_default_value)
            .or_else(|| recurrence_choices.display_for_value(&RecurrenceChoice::None))
            .unwrap_or_else(|| "None".into());
        defaults.insert("recurrence".into(), recurrence_default_display);

        if let Some(display) = status_choices.display_for_value(&default_status) {
            defaults.insert("status".into(), display);
        }

        match &mode {
            TransactionWizardMode::Create { .. } => {
                defaults.insert("actual_date".into(), String::new());
                defaults.insert("actual_amount".into(), String::new());
            }
            TransactionWizardMode::Edit { initial } => {
                if let Some(display) = account_choices.display_for_value(&initial.from_account) {
                    defaults.insert("from_account".into(), display);
                }
                if let Some(display) = account_choices.display_for_value(&initial.to_account) {
                    defaults.insert("to_account".into(), display);
                }
                defaults.insert("scheduled_date".into(), initial.scheduled_date.to_string());
                if let Some(actual_date) = initial.actual_date {
                    defaults.insert("actual_date".into(), actual_date.to_string());
                } else {
                    defaults.insert("actual_date".into(), String::new());
                }
                defaults.insert(
                    "budgeted_amount".into(),
                    format_amount(initial.budgeted_amount),
                );
                if let Some(actual) = initial.actual_amount {
                    defaults.insert("actual_amount".into(), format_amount(actual));
                } else {
                    defaults.insert("actual_amount".into(), String::new());
                }
                if let Some(category_id) = initial.category_id {
                    if let Some(display) = category_choices.display_for_value(&Some(category_id)) {
                        defaults.insert("category".into(), display);
                    }
                }
                if let Some(notes) = &initial.notes {
                    defaults.insert("notes".into(), notes.clone());
                } else {
                    defaults.insert("notes".into(), String::new());
                }
                if recurrence_default_value == RecurrenceChoice::EveryNDays {
                    defaults.insert(
                        "recurrence_days".into(),
                        recurrence_every_default.to_string(),
                    );
                }
            }
        }

        if let Some(field) = fields.iter_mut().find(|f| f.key == "actual_amount") {
            let default_amount = match &mode {
                TransactionWizardMode::Create { .. } => None,
                TransactionWizardMode::Edit { initial } => {
                    Some(initial.actual_amount.unwrap_or(initial.budgeted_amount))
                }
            };
            field.validator = make_optional_non_negative_decimal_validator(default_amount);
        }

        let descriptor = FormDescriptor::new("transaction", fields);

        Self {
            descriptor,
            defaults,
            mode,
            account_choices,
            category_choices,
            recurrence_choices,
            status_choices,
        }
    }

    fn apply_existing_recurrence_metadata(&self, recurrence: &mut Recurrence) {
        if let TransactionWizardMode::Edit { initial } = &self.mode {
            if let Some(existing) = &initial.recurrence {
                recurrence.series_id = existing.series_id;
                recurrence.mode = existing.mode.clone();
                recurrence.end = existing.end.clone();
                recurrence.exceptions = existing.exceptions.clone();
                recurrence.status = existing.status.clone();
                recurrence.last_generated = existing.last_generated;
                recurrence.last_completed = existing.last_completed;
                recurrence.generated_occurrences = existing.generated_occurrences;
                recurrence.next_scheduled = existing.next_scheduled;
            }
        }
    }

    fn existing_id(&self) -> Option<Uuid> {
        if let TransactionWizardMode::Edit { initial } = &self.mode {
            Some(initial.id)
        } else {
            None
        }
    }
}

impl FormFlow for TransactionWizard {
    type Output = TransactionFormData;
    type Error = Infallible;

    fn descriptor(&self) -> &FormDescriptor {
        &self.descriptor
    }

    fn defaults(&self) -> BTreeMap<String, String> {
        self.defaults.clone()
    }

    fn commit(&self, values: BTreeMap<String, String>) -> Result<Self::Output, Self::Error> {
        let from_display = values
            .get("from_account")
            .cloned()
            .or_else(|| self.defaults.get("from_account").cloned())
            .unwrap_or_default();
        let from_account = self
            .account_choices
            .value_for_display(&from_display)
            .cloned()
            .unwrap_or_else(Uuid::new_v4);

        let to_display = values
            .get("to_account")
            .cloned()
            .or_else(|| self.defaults.get("to_account").cloned())
            .unwrap_or_default();
        let to_account = self
            .account_choices
            .value_for_display(&to_display)
            .cloned()
            .unwrap_or_else(Uuid::new_v4);

        let category_display = values
            .get("category")
            .cloned()
            .or_else(|| self.defaults.get("category").cloned())
            .unwrap_or_else(|| "None".into());
        let category_id = self
            .category_choices
            .value_for_display(&category_display)
            .cloned()
            .unwrap_or(None);

        let scheduled_raw = values
            .get("scheduled_date")
            .cloned()
            .or_else(|| self.defaults.get("scheduled_date").cloned())
            .unwrap_or_else(|| NaiveDate::default().to_string());
        let scheduled_date = NaiveDate::parse_from_str(&scheduled_raw, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

        let actual_date = values
            .get("actual_date")
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok());

        let budget_raw = values
            .get("budgeted_amount")
            .cloned()
            .or_else(|| self.defaults.get("budgeted_amount").cloned())
            .unwrap_or_else(|| "0".into());
        let budgeted_amount = budget_raw.parse::<f64>().unwrap_or(0.0);

        let actual_amount = values.get("actual_amount").cloned().and_then(|value| {
            if value.trim().is_empty() {
                None
            } else {
                parse_optional_f64(&value)
            }
        });

        let recurrence_display = values
            .get("recurrence")
            .cloned()
            .or_else(|| self.defaults.get("recurrence").cloned())
            .unwrap_or_else(|| "None".into());
        let recurrence_choice = self
            .recurrence_choices
            .value_for_display(&recurrence_display)
            .cloned()
            .unwrap_or(RecurrenceChoice::None);

        let recurrence_days_raw = values
            .get("recurrence_days")
            .cloned()
            .or_else(|| self.defaults.get("recurrence_days").cloned())
            .unwrap_or_else(|| "1".into());
        let recurrence_days = recurrence_days_raw
            .trim()
            .parse::<u32>()
            .unwrap_or(1)
            .max(1);

        let status_display = values
            .get("status")
            .cloned()
            .or_else(|| self.defaults.get("status").cloned())
            .unwrap_or_else(|| "Planned".into());
        let status = self
            .status_choices
            .value_for_display(&status_display)
            .cloned()
            .unwrap_or(TransactionStatus::Planned);

        let notes = values.get("notes").and_then(|value| sanitize_notes(value));

        let recurrence_action = match recurrence_choice {
            RecurrenceChoice::None => TransactionRecurrenceAction::Clear,
            RecurrenceChoice::EveryNDays => {
                let mut recurrence = Recurrence::new(
                    scheduled_date,
                    TimeInterval {
                        every: recurrence_days,
                        unit: TimeUnit::Day,
                    },
                    RecurrenceMode::FixedSchedule,
                );
                self.apply_existing_recurrence_metadata(&mut recurrence);
                TransactionRecurrenceAction::Set(recurrence)
            }
            RecurrenceChoice::Preset(interval) => {
                let mut recurrence =
                    Recurrence::new(scheduled_date, interval, RecurrenceMode::FixedSchedule);
                self.apply_existing_recurrence_metadata(&mut recurrence);
                TransactionRecurrenceAction::Set(recurrence)
            }
            RecurrenceChoice::KeepExisting => TransactionRecurrenceAction::Keep,
        };

        let id = self.existing_id();

        Ok(TransactionFormData {
            id,
            from_account,
            to_account,
            category_id,
            scheduled_date,
            actual_date,
            budgeted_amount,
            actual_amount,
            status,
            notes,
            recurrence: recurrence_action,
        })
    }

    fn cancel(&self) -> Self::Error {
        unreachable!()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CategoryFormData {
    pub id: Option<Uuid>,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<Uuid>,
    pub is_custom: bool,
    pub notes: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CategoryInitialData {
    pub id: Uuid,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<Uuid>,
    pub is_custom: bool,
    pub notes: Option<String>,
}

#[derive(Clone, Debug)]
enum CategoryWizardMode {
    Create,
    Edit {
        id: Uuid,
        locked_kind: CategoryKind,
        locked_custom: bool,
    },
}

pub struct CategoryWizard {
    descriptor: FormDescriptor,
    defaults: BTreeMap<String, String>,
    mode: CategoryWizardMode,
    kind_choices: Option<ChoiceMapper<CategoryKind>>,
    parent_choices: ChoiceMapper<Option<Uuid>>,
    custom_choices: Option<ChoiceMapper<bool>>,
}

impl CategoryWizard {
    pub fn new_create(
        existing_names: HashSet<String>,
        parent_options: Vec<(String, Option<Uuid>)>,
    ) -> Self {
        Self::build(existing_names, None, parent_options, true, true)
    }

    pub fn new_edit(
        existing_names: HashSet<String>,
        initial: CategoryInitialData,
        parent_options: Vec<(String, Option<Uuid>)>,
        allow_kind_change: bool,
        allow_custom_change: bool,
    ) -> Self {
        Self::build(
            existing_names,
            Some(initial),
            parent_options,
            allow_kind_change,
            allow_custom_change,
        )
    }

    fn build(
        existing_names: HashSet<String>,
        initial: Option<CategoryInitialData>,
        parent_options: Vec<(String, Option<Uuid>)>,
        allow_kind_change: bool,
        allow_custom_change: bool,
    ) -> Self {
        let mut name_set: HashSet<String> = existing_names
            .into_iter()
            .map(|value| value.to_ascii_lowercase())
            .collect();

        if let Some(data) = &initial {
            name_set.remove(&normalize_name(&data.name));
        }

        let name_validator = make_name_validator(name_set);

        let kind_pairs = vec![
            ("Expense".to_string(), CategoryKind::Expense),
            ("Income".to_string(), CategoryKind::Income),
            ("Transfer".to_string(), CategoryKind::Transfer),
        ];
        let kind_choices = if allow_kind_change {
            Some(ChoiceMapper::from_pairs(kind_pairs))
        } else {
            None
        };

        let mut parent_pairs = Vec::new();
        parent_pairs.push(("None".to_string(), None));
        for (label, id) in parent_options {
            parent_pairs.push((label, id));
        }
        let parent_choices = ChoiceMapper::from_pairs(parent_pairs);

        let custom_pairs = vec![
            ("Custom (user-defined)".to_string(), true),
            ("Predefined (protected)".to_string(), false),
        ];
        let custom_choices = if allow_custom_change {
            Some(ChoiceMapper::from_pairs(custom_pairs))
        } else {
            None
        };

        let mut fields = Vec::new();
        fields.push(FieldDescriptor::new(
            "name",
            "Category name",
            FieldKind::Text,
            name_validator,
        ));

        if let Some(mapper) = kind_choices.clone() {
            fields.push(FieldDescriptor::new(
                "kind",
                "Category type",
                FieldKind::Choice(mapper.options()),
                make_choice_validator(mapper, "category type"),
            ));
        }

        fields.push(
            FieldDescriptor::new(
                "parent",
                "Parent category",
                FieldKind::Choice(parent_choices.options()),
                make_choice_validator(parent_choices.clone(), "parent category"),
            )
            .with_optional(),
        );

        if let Some(mapper) = custom_choices.clone() {
            fields.push(FieldDescriptor::new(
                "custom",
                "Custom or predefined",
                FieldKind::Choice(mapper.options()),
                make_choice_validator(mapper, "custom flag"),
            ));
        }

        fields.push(
            FieldDescriptor::new("notes", "Notes", FieldKind::Text, make_notes_validator(512))
                .with_optional(),
        );

        let mut defaults = BTreeMap::new();
        let mode = if let Some(data) = initial {
            defaults.insert("name".into(), data.name.clone());
            if let Some(mapper) = &kind_choices {
                if let Some(display) = mapper.display_for_value(&data.kind) {
                    defaults.insert("kind".into(), display);
                }
            }
            if let Some(display) = parent_choices.display_for_value(&data.parent_id) {
                defaults.insert("parent".into(), display);
            }
            if let Some(mapper) = &custom_choices {
                if let Some(display) = mapper.display_for_value(&data.is_custom) {
                    defaults.insert("custom".into(), display);
                }
            }
            if let Some(notes) = data.notes {
                defaults.insert("notes".into(), notes);
            }
            CategoryWizardMode::Edit {
                id: data.id,
                locked_kind: data.kind,
                locked_custom: data.is_custom,
            }
        } else {
            if let Some(mapper) = &kind_choices {
                if let Some(display) = mapper.display_for_value(&CategoryKind::Expense) {
                    defaults.insert("kind".into(), display);
                }
            }
            if let Some(display) = parent_choices.display_for_value(&None) {
                defaults.insert("parent".into(), display);
            }
            if let Some(mapper) = &custom_choices {
                if let Some(display) = mapper.display_for_value(&true) {
                    defaults.insert("custom".into(), display);
                }
            }
            CategoryWizardMode::Create
        };

        Self {
            descriptor: FormDescriptor::new("category", fields),
            defaults,
            mode,
            kind_choices,
            parent_choices,
            custom_choices,
        }
    }
}

impl FormFlow for CategoryWizard {
    type Output = CategoryFormData;
    type Error = Infallible;

    fn descriptor(&self) -> &FormDescriptor {
        &self.descriptor
    }

    fn defaults(&self) -> BTreeMap<String, String> {
        self.defaults.clone()
    }

    fn commit(&self, values: BTreeMap<String, String>) -> Result<Self::Output, Self::Error> {
        let name = values.get("name").cloned().unwrap_or_default();

        let kind = if let Some(mapper) = &self.kind_choices {
            let display = values
                .get("kind")
                .cloned()
                .or_else(|| self.defaults.get("kind").cloned())
                .and_then(|value| mapper.value_for_display(&value).cloned())
                .unwrap_or(CategoryKind::Expense);
            display
        } else {
            match self.mode {
                CategoryWizardMode::Edit {
                    ref locked_kind, ..
                } => locked_kind.clone(),
                CategoryWizardMode::Create => CategoryKind::Expense,
            }
        };

        let parent_display = values
            .get("parent")
            .cloned()
            .or_else(|| self.defaults.get("parent").cloned())
            .or_else(|| self.parent_choices.display_for_value(&None))
            .unwrap_or_else(|| "None".into());
        let parent_id = self
            .parent_choices
            .value_for_display(&parent_display)
            .cloned()
            .unwrap_or(None);

        let is_custom = if let Some(mapper) = &self.custom_choices {
            values
                .get("custom")
                .cloned()
                .or_else(|| self.defaults.get("custom").cloned())
                .and_then(|value| mapper.value_for_display(&value).cloned())
                .unwrap_or(true)
        } else {
            match self.mode {
                CategoryWizardMode::Edit { locked_custom, .. } => locked_custom,
                CategoryWizardMode::Create => true,
            }
        };

        let notes = values.get("notes").and_then(|value| sanitize_notes(value));

        let id = match self.mode {
            CategoryWizardMode::Create => None,
            CategoryWizardMode::Edit { id, .. } => Some(id),
        };

        Ok(CategoryFormData {
            id,
            name,
            kind,
            parent_id,
            is_custom,
            notes,
        })
    }

    fn cancel(&self) -> Self::Error {
        unreachable!()
    }
}

/// Snapshot of collected data displayed before final confirmation.
#[derive(Default)]
pub struct FormSummary {
    pub entries: Vec<(String, String)>,
}

/// Interaction surface used by the form engine. The CLI will provide an
/// implementation that leverages dialoguer and the shared output module.
pub struct PromptContext<'a> {
    pub descriptor: &'a FieldDescriptor,
    pub default: Option<&'a str>,
    pub index: usize,
    pub total: usize,
}

pub trait FormInteraction {
    fn prompt_field(&mut self, context: &PromptContext<'_>) -> PromptResponse;

    fn confirm(&mut self, summary: &FormSummary, lines: &[String]) -> ConfirmationResponse;
}

/// Interactive implementation that relies on the shared menu renderer and
/// prompt components for consistent UX.
pub struct WizardInteraction;

impl WizardInteraction {
    pub fn new() -> Self {
        Self
    }

    fn prompt_text(&mut self, context: &PromptContext<'_>) -> PromptResponse {
        self.print_step_header(context);
        match text_input(context.descriptor.label, context.default) {
            Ok(TextPromptResult::Value(value)) => PromptResponse::Value(value),
            Ok(TextPromptResult::Keep) => PromptResponse::Keep,
            Ok(TextPromptResult::Back) => PromptResponse::Back,
            Ok(TextPromptResult::Help) => PromptResponse::Help,
            Ok(TextPromptResult::Escape) => {
                if context.index == 0 {
                    PromptResponse::Cancel
                } else {
                    PromptResponse::Back
                }
            }
            Ok(TextPromptResult::Cancel) | Err(_) => PromptResponse::Cancel,
        }
    }

    fn prompt_choice(&mut self, context: &PromptContext<'_>, options: &[String]) -> PromptResponse {
        let mut lines = self.choice_context_lines(context);
        if let Some(help) = context.descriptor.help {
            lines.push(help.to_string());
        }
        let title = self.step_title(context);
        match choice_menu(&title, &lines, options, context.default, context.index > 0) {
            Ok(ChoicePromptResult::Value(value)) => {
                if context
                    .default
                    .map(|d| d.eq_ignore_ascii_case(&value))
                    .unwrap_or(false)
                {
                    PromptResponse::Keep
                } else {
                    PromptResponse::Value(value)
                }
            }
            Ok(ChoicePromptResult::Back) => PromptResponse::Back,
            _ => PromptResponse::Cancel,
        }
    }

    fn prompt_boolean(&mut self, context: &PromptContext<'_>) -> PromptResponse {
        let options = vec!["Yes".to_string(), "No".to_string()];
        let default_label = context.default.map(|value| {
            if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") {
                "Yes"
            } else {
                "No"
            }
        });
        let mut lines = self.choice_context_lines(context);
        if let Some(help) = context.descriptor.help {
            lines.push(help.to_string());
        }
        let title = self.step_title(context);
        match choice_menu(&title, &lines, &options, default_label, context.index > 0) {
            Ok(ChoicePromptResult::Value(choice)) => {
                let bool_value = if choice.eq_ignore_ascii_case("yes") {
                    "true"
                } else {
                    "false"
                };
                if context
                    .default
                    .map(|d| d.eq_ignore_ascii_case(bool_value))
                    .unwrap_or(false)
                {
                    PromptResponse::Keep
                } else {
                    PromptResponse::Value(bool_value.to_string())
                }
            }
            Ok(ChoicePromptResult::Back) => PromptResponse::Back,
            _ => PromptResponse::Cancel,
        }
    }

    fn choice_context_lines(&self, context: &PromptContext<'_>) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(default) = context.default {
            lines.push(format!("Default: {}", default));
        }
        lines.push("Use ↑ ↓ to highlight an option, Enter to select.".into());
        lines.push("Press ESC to cancel.".into());
        if context.index > 0 {
            lines.push("Select ← Back to revisit the previous field.".into());
        }
        lines
    }

    fn step_title(&self, context: &PromptContext<'_>) -> String {
        format!(
            "Step {} / {} — {}",
            context.index + 1,
            context.total,
            context.descriptor.label
        )
    }

    fn print_step_header(&self, context: &PromptContext<'_>) {
        let title = self.step_title(context);
        let header = format_header(&title);
        let ui = style();
        let rule = ui.horizontal_line(visible_width(&header));
        println!("{header}");
        println!("{rule}");
        println!();
    }
}

impl FormInteraction for WizardInteraction {
    fn prompt_field(&mut self, context: &PromptContext<'_>) -> PromptResponse {
        match &context.descriptor.kind {
            FieldKind::Choice(options) => self.prompt_choice(context, options),
            FieldKind::Boolean => self.prompt_boolean(context),
            _ => self.prompt_text(context),
        }
    }

    fn confirm(&mut self, _summary: &FormSummary, lines: &[String]) -> ConfirmationResponse {
        let mut context_lines = Vec::new();
        context_lines.extend_from_slice(lines);
        context_lines.push(String::new());
        context_lines.push(
            "Use the menu below to confirm, edit the previous field, or cancel. ESC cancels."
                .into(),
        );
        match confirm_menu(&context_lines) {
            Ok(ConfirmationPromptResult::Confirm) => ConfirmationResponse::Confirm,
            Ok(ConfirmationPromptResult::Back) => ConfirmationResponse::Back,
            _ => ConfirmationResponse::Cancel,
        }
    }
}

/// Represents an in-progress wizard session. Callers may drive the session
/// manually (start/next/validate) or use [`FormEngine::run`] to handle the full
/// loop automatically.
pub struct FormSession<'a> {
    descriptor: &'a FormDescriptor,
    values: BTreeMap<String, String>,
    defaults: BTreeMap<String, String>,
    index: usize,
    completed: bool,
    cancelled: bool,
}

impl<'a> FormSession<'a> {
    pub fn new(descriptor: &'a FormDescriptor, defaults: BTreeMap<String, String>) -> Self {
        Self {
            descriptor,
            values: defaults.clone(),
            defaults,
            index: 0,
            completed: false,
            cancelled: false,
        }
    }

    pub fn start(&mut self) -> Option<FormStep<'_>> {
        self.index = 0;
        self.current_field()
    }

    pub fn current_field(&self) -> Option<FormStep<'_>> {
        self.descriptor
            .fields
            .get(self.index)
            .map(|field| FormStep {
                descriptor: field,
                default: self
                    .values
                    .get(field.key)
                    .cloned()
                    .or_else(|| self.defaults.get(field.key).cloned()),
                index: self.index,
                total: self.descriptor.fields.len(),
            })
    }

    pub fn apply_response(
        &mut self,
        response: PromptResponse,
    ) -> Result<FormSessionEvent, ValidationError> {
        if self.completed || self.cancelled {
            return Ok(FormSessionEvent::NoOp);
        }

        let Some(field) = self.descriptor.fields.get(self.index) else {
            return Ok(FormSessionEvent::NoOp);
        };

        match response {
            PromptResponse::Cancel => {
                self.cancelled = true;
                Ok(FormSessionEvent::Cancelled)
            }
            PromptResponse::Back => {
                if self.index > 0 {
                    self.index -= 1;
                    Ok(FormSessionEvent::Moved)
                } else {
                    io::print_warning("Already at the first field.");
                    Ok(FormSessionEvent::Repeat)
                }
            }
            PromptResponse::Help => {
                if let Some(help) = field.help {
                    io::print_info(help);
                } else {
                    io::print_info("No additional information available for this field.");
                }
                Ok(FormSessionEvent::Repeat)
            }
            PromptResponse::Keep => {
                if let Some(existing) = self.values.get(field.key) {
                    self.values.insert(field.key.to_string(), existing.clone());
                    self.index += 1;
                    Ok(FormSessionEvent::Moved)
                } else if field.required {
                    io::print_warning("This field is required.");
                    Ok(FormSessionEvent::Repeat)
                } else {
                    self.values.remove(field.key);
                    self.index += 1;
                    Ok(FormSessionEvent::Moved)
                }
            }
            PromptResponse::Value(raw) => match self.validate_field(field, &raw) {
                Ok(value) => {
                    self.values.insert(field.key.to_string(), value);
                    self.index += 1;
                    Ok(FormSessionEvent::Moved)
                }
                Err(err) => {
                    io::print_warning(&err.message);
                    Err(err)
                }
            },
        }
    }

    fn validate_field(
        &self,
        field: &FieldDescriptor,
        raw: &str,
    ) -> Result<String, ValidationError> {
        match (&field.kind, &field.validator) {
            (FieldKind::Choice(options), Validator::None) => {
                let normalized = raw.trim().to_lowercase();
                options
                    .iter()
                    .find(|candidate| candidate.to_lowercase() == normalized)
                    .cloned()
                    .ok_or_else(|| {
                        ValidationError::new(format!(
                            "Value must be one of: {}",
                            options.join(", ")
                        ))
                    })
            }
            (FieldKind::Boolean, Validator::None) => {
                let normalized = raw.trim().to_lowercase();
                match normalized.as_str() {
                    "y" | "yes" | "true" | "1" => Ok("true".into()),
                    "n" | "no" | "false" | "0" => Ok("false".into()),
                    _ => Err(ValidationError::new(
                        "Enter yes/no, true/false, or 1/0 to indicate boolean values",
                    )),
                }
            }
            (kind, validator @ Validator::None) => match kind {
                FieldKind::Integer => Validator::Integer.validate(raw),
                FieldKind::Decimal => Validator::Decimal.validate(raw),
                FieldKind::Date => Validator::Date.validate(raw),
                FieldKind::Time => Validator::Time.validate(raw),
                FieldKind::Choice(_) => validator.validate(raw),
                _ => Ok(raw.trim().to_string()),
            },
            (_, validator) => validator.validate(raw),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.index >= self.descriptor.fields.len()
    }

    pub fn mark_complete(&mut self) {
        self.completed = true;
    }

    pub fn mark_cancelled(&mut self) {
        self.cancelled = true;
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormSessionEvent {
    Moved,
    Repeat,
    Cancelled,
    NoOp,
}

/// Data provided to render the current field.
pub struct FormStep<'a> {
    pub descriptor: &'a FieldDescriptor,
    pub default: Option<String>,
    pub index: usize,
    pub total: usize,
}

impl<'a> FormStep<'a> {
    pub fn default_value(&self) -> Option<&str> {
        self.default.as_deref()
    }
}

/// High-level form contract for entity-specific wizards.
///
/// Implementations describe their field descriptor, supply defaults (when
/// editing existing entities), and transform collected values into an output
/// type once the wizard completes.
pub trait FormFlow {
    type Output;
    type Error;

    /// Descriptor controlling field order and behaviour.
    fn descriptor(&self) -> &FormDescriptor;

    /// Default values used to pre-populate the session.
    fn defaults(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Builds the concrete output after successful completion.
    fn commit(&self, values: BTreeMap<String, String>) -> Result<Self::Output, Self::Error>;

    /// Converts cancellation into domain-friendly error signalling.
    fn cancel(&self) -> Self::Error;
}

/// Drives a [`FormFlow`] using an [`FormInteraction`] implementation.
pub struct FormEngine<'a, F: FormFlow> {
    flow: &'a F,
}

impl<'a, F: FormFlow> FormEngine<'a, F> {
    pub fn new(flow: &'a F) -> Self {
        Self { flow }
    }

    pub fn run<I: FormInteraction>(
        &self,
        interaction: &mut I,
    ) -> Result<FormResult<F::Output>, F::Error> {
        let descriptor = self.flow.descriptor();
        let mut session = FormSession::new(descriptor, self.flow.defaults());

        session.start();

        loop {
            if session.cancelled {
                return Ok(FormResult::Cancelled);
            }

            if session.is_complete() {
                let summary = build_summary(descriptor, session.values());
                let summary_lines = format_summary_lines(&summary);
                match interaction.confirm(&summary, &summary_lines) {
                    ConfirmationResponse::Confirm => {
                        session.mark_complete();
                        let output = self.flow.commit(session.values().clone())?;
                        return Ok(FormResult::Completed(output));
                    }
                    ConfirmationResponse::Back => {
                        session.mark_complete();
                        if descriptor.fields.is_empty() {
                            return Ok(FormResult::Cancelled);
                        }
                        // Respect back by showing last field again.
                        let last_index = descriptor.fields.len().saturating_sub(1);
                        session.index = last_index;
                        continue;
                    }
                    ConfirmationResponse::Cancel => {
                        session.mark_cancelled();
                        return Ok(FormResult::Cancelled);
                    }
                }
            }

            let Some(step) = session.current_field() else {
                // No fields to process: treat as immediate completion.
                session.mark_complete();
                continue;
            };

            let response = {
                let context = PromptContext {
                    descriptor: step.descriptor,
                    default: step.default_value(),
                    index: step.index,
                    total: step.total,
                };
                if !matches!(context.descriptor.kind, FieldKind::Choice(_)) {
                    render_prompt(&context);
                }
                interaction.prompt_field(&context)
            };
            if let PromptResponse::Cancel = response {
                session.mark_cancelled();
                return Ok(FormResult::Cancelled);
            }

            // Apply response and handle validation errors.
            match session.apply_response(response) {
                Ok(FormSessionEvent::Moved | FormSessionEvent::Repeat | FormSessionEvent::NoOp) => {
                    continue;
                }
                Ok(FormSessionEvent::Cancelled) => {
                    return Ok(FormResult::Cancelled);
                }
                Err(_) => {
                    // Validation errors are already logged; simply re-loop.
                    continue;
                }
            }
        }
    }
}

fn render_prompt(context: &PromptContext<'_>) {
    let formatter = Formatter::new();
    formatter.print_header(format!(
        "Step {} of {} – {}",
        context.index + 1,
        context.total,
        context.descriptor.label
    ));
    if let Some(default_value) = context.default {
        formatter.print_detail(format!("Default: {}", default_value));
    }
    if let Some(help) = context.descriptor.help {
        formatter.print_detail(help);
    }
    let mut instructions = vec!["Type a value and press Enter to continue.".to_string()];
    if context.index == 0 {
        instructions.push("Press ESC to cancel the wizard.".into());
    } else {
        instructions.push("Press ESC to return to the previous field.".into());
    }
    instructions.push("Type :help for details or :clear to remove the current value.".into());
    if context.index > 0 {
        instructions.push("Type :back to revisit the previous field.".into());
    }
    formatter.print_detail(instructions.join(" "));
}

fn format_summary_lines(summary: &FormSummary) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("Review your entries:".into());
    for (key, value) in &summary.entries {
        lines.push(format!("  {}: {}", key, value));
    }
    lines
}

fn build_summary(descriptor: &FormDescriptor, values: &BTreeMap<String, String>) -> FormSummary {
    let mut summary = FormSummary::default();
    for field in &descriptor.fields {
        if let Some(value) = values.get(field.key) {
            summary
                .entries
                .push((field.label.to_string(), value.to_string()));
        } else {
            summary
                .entries
                .push((field.label.to_string(), "[unfilled]".to_string()));
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::OnceLock;
    use uuid::Uuid;

    struct MockInteraction {
        prompts: std::collections::VecDeque<PromptResponse>,
        confirmations: std::collections::VecDeque<ConfirmationResponse>,
        help_hits: usize,
    }

    impl MockInteraction {
        fn new(prompts: Vec<PromptResponse>, confirmations: Vec<ConfirmationResponse>) -> Self {
            Self {
                prompts: prompts.into(),
                confirmations: confirmations.into(),
                help_hits: 0,
            }
        }
    }

    impl FormInteraction for MockInteraction {
        fn prompt_field(&mut self, _context: &PromptContext<'_>) -> PromptResponse {
            let response = self
                .prompts
                .pop_front()
                .unwrap_or(PromptResponse::Value("".into()));
            if matches!(response, PromptResponse::Help) {
                self.help_hits += 1;
            }
            response
        }

        fn confirm(&mut self, _summary: &FormSummary, _lines: &[String]) -> ConfirmationResponse {
            self.confirmations
                .pop_front()
                .unwrap_or(ConfirmationResponse::Confirm)
        }
    }

    struct TestForm;

    impl FormFlow for TestForm {
        type Output = BTreeMap<String, String>;
        type Error = &'static str;

        fn descriptor(&self) -> &FormDescriptor {
            static DESCRIPTOR: OnceLock<FormDescriptor> = OnceLock::new();
            DESCRIPTOR.get_or_init(|| {
                FormDescriptor::new(
                    "test",
                    vec![
                        FieldDescriptor::new("name", "Name", FieldKind::Text, Validator::NonEmpty),
                        FieldDescriptor::new(
                            "amount",
                            "Amount",
                            FieldKind::Decimal,
                            Validator::PositiveNumber,
                        )
                        .with_help("Enter the amount in the base currency."),
                    ],
                )
            })
        }

        fn commit(&self, values: BTreeMap<String, String>) -> Result<Self::Output, Self::Error> {
            Ok(values)
        }

        fn cancel(&self) -> Self::Error {
            "cancelled"
        }
    }

    #[test]
    fn wizard_completes_successfully() {
        let form = TestForm;
        let engine = FormEngine::new(&form);
        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Value("Groceries".into()),
                PromptResponse::Value("150.25".into()),
            ],
            vec![ConfirmationResponse::Confirm],
        );

        let result = engine.run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(values) => {
                assert_eq!(values.get("name").unwrap(), "Groceries");
                assert_eq!(values.get("amount").unwrap(), "150.25");
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn wizard_reprompts_on_invalid_input() {
        let form = TestForm;
        let engine = FormEngine::new(&form);
        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Value("".into()),     // invalid
                PromptResponse::Value("Rent".into()), // valid
                PromptResponse::Value("-5".into()),   // invalid
                PromptResponse::Value("5".into()),    // valid
            ],
            vec![ConfirmationResponse::Confirm],
        );

        let result = engine.run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(values) => {
                assert_eq!(values.get("name").unwrap(), "Rent");
                assert_eq!(values.get("amount").unwrap(), "5");
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn wizard_cancelled_midway() {
        let form = TestForm;
        let engine = FormEngine::new(&form);
        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Value("Utilities".into()),
                PromptResponse::Cancel,
            ],
            vec![],
        );

        let result = engine.run(&mut interaction).unwrap();
        assert!(matches!(result, FormResult::Cancelled));
    }

    #[test]
    fn wizard_supports_back_command() {
        let form = TestForm;
        let engine = FormEngine::new(&form);
        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Value("Lunch".into()),
                PromptResponse::Back, // go back from second field
                PromptResponse::Value("Lunch".into()), // re-enter first field
                PromptResponse::Value("12.00".into()),
            ],
            vec![ConfirmationResponse::Confirm],
        );

        let result = engine.run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(values) => {
                assert_eq!(values.get("name").unwrap(), "Lunch");
                assert_eq!(values.get("amount").unwrap(), "12");
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn wizard_help_does_not_progress() {
        let form = TestForm;
        let engine = FormEngine::new(&form);
        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Help,
                PromptResponse::Value("Snacks".into()),
                PromptResponse::Value("25".into()),
            ],
            vec![ConfirmationResponse::Confirm],
        );

        let result = engine.run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(values) => {
                assert_eq!(values.get("name").unwrap(), "Snacks");
                assert_eq!(values.get("amount").unwrap(), "25");
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn account_wizard_collects_all_fields() {
        let wizard = AccountWizard::new_create(HashSet::new(), Vec::new());
        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Value("Checking".into()),
                PromptResponse::Value("1".into()),
                PromptResponse::Keep,
                PromptResponse::Value("500".into()),
                PromptResponse::Value("Primary checking".into()),
            ],
            vec![ConfirmationResponse::Confirm],
        );

        let result = FormEngine::new(&wizard).run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(data) => {
                assert_eq!(data.id, None);
                assert_eq!(data.name, "Checking");
                assert_eq!(data.kind, AccountKind::Bank);
                assert_eq!(data.category_id, None);
                assert_eq!(data.opening_balance, Some(500.0));
                assert_eq!(data.notes.as_deref(), Some("Primary checking"));
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn transaction_wizard_create_every_n_days() {
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();
        let accounts = vec![("From".to_string(), from_id), ("To".to_string(), to_id)];
        let categories = Vec::new();
        let today = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let min_date = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let wizard = TransactionWizard::new_create(
            accounts,
            categories,
            today,
            min_date,
            TransactionStatus::Planned,
        );

        let prompts = vec![
            PromptResponse::Value("1".into()), // from account
            PromptResponse::Value("2".into()), // to account
            PromptResponse::Keep,              // category none
            PromptResponse::Value("2025-03-01".into()),
            PromptResponse::Keep, // actual date empty
            PromptResponse::Value("950".into()),
            PromptResponse::Keep,              // actual amount blank
            PromptResponse::Value("5".into()), // Every N days
            PromptResponse::Value("3".into()), // N = 3
            PromptResponse::Keep,              // status Planned
            PromptResponse::Value("Rent".into()),
        ];
        let mut interaction = MockInteraction::new(prompts, vec![ConfirmationResponse::Confirm]);

        let result = FormEngine::new(&wizard).run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(data) => {
                assert!(data.id.is_none());
                assert_eq!(data.from_account, from_id);
                assert_eq!(data.to_account, to_id);
                assert_eq!(data.category_id, None);
                assert_eq!(
                    data.scheduled_date,
                    NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()
                );
                assert!(data.actual_date.is_none());
                assert_eq!(data.budgeted_amount, 950.0);
                assert!(data.actual_amount.is_none());
                assert_eq!(data.status, TransactionStatus::Planned);
                assert_eq!(data.notes.as_deref(), Some("Rent"));
                match data.recurrence {
                    TransactionRecurrenceAction::Set(recurrence) => {
                        assert_eq!(recurrence.interval.every, 3);
                        assert_eq!(recurrence.interval.unit, TimeUnit::Day);
                        assert_eq!(recurrence.start_date, data.scheduled_date);
                    }
                    other => panic!("Unexpected recurrence action: {:?}", other),
                }
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn transaction_wizard_edit_keep_existing() {
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let accounts = vec![("From".to_string(), from_id), ("To".to_string(), to_id)];
        let categories = vec![("Cat".to_string(), Some(category_id))];
        let scheduled = NaiveDate::from_ymd_opt(2024, 5, 1).unwrap();
        let recurrence = Recurrence::new(
            scheduled,
            TimeInterval {
                every: 2,
                unit: TimeUnit::Week,
            },
            RecurrenceMode::FixedSchedule,
        );
        let initial = TransactionInitialData {
            id: Uuid::new_v4(),
            from_account: from_id,
            to_account: to_id,
            category_id: Some(category_id),
            scheduled_date: scheduled,
            actual_date: Some(NaiveDate::from_ymd_opt(2024, 5, 2).unwrap()),
            budgeted_amount: 100.0,
            actual_amount: Some(105.0),
            recurrence: Some(recurrence),
            status: TransactionStatus::Planned,
            notes: Some("Initial".into()),
        };
        let initial_id = initial.id;
        let wizard = TransactionWizard::new_edit(
            accounts,
            categories,
            NaiveDate::from_ymd_opt(2024, 5, 10).unwrap(),
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            initial,
        );

        let prompts = vec![
            PromptResponse::Keep,                       // from account
            PromptResponse::Keep,                       // to account
            PromptResponse::Keep,                       // category
            PromptResponse::Value("2024-06-01".into()), // scheduled date
            PromptResponse::Keep,                       // actual date
            PromptResponse::Value("125.50".into()),
            PromptResponse::Value("120.25".into()),
            PromptResponse::Keep,              // keep existing recurrence
            PromptResponse::Keep,              // recurrence days unused
            PromptResponse::Value("3".into()), // status -> Missed
            PromptResponse::Value("Updated".into()),
        ];
        let mut interaction = MockInteraction::new(prompts, vec![ConfirmationResponse::Confirm]);

        let result = FormEngine::new(&wizard).run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(data) => {
                assert_eq!(data.id, Some(initial_id));
                assert_eq!(
                    data.scheduled_date,
                    NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()
                );
                assert_eq!(data.budgeted_amount, 125.50);
                assert_eq!(data.actual_amount, Some(120.25));
                assert_eq!(data.status, TransactionStatus::Missed);
                assert_eq!(data.notes.as_deref(), Some("Updated"));
                assert!(matches!(data.recurrence, TransactionRecurrenceAction::Keep));
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn category_wizard_respects_locked_fields() {
        let category_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let initial = CategoryInitialData {
            id: category_id,
            name: "Rent".into(),
            kind: CategoryKind::Expense,
            parent_id: None,
            is_custom: false,
            notes: Some("Fixed".into()),
        };
        let existing_names: HashSet<String> = HashSet::from(["Rent".into(), "Utilities".into()]);
        let mut shorthand = parent_id.simple().to_string();
        shorthand.truncate(8);
        let parent_label = format!("Housing (Expense) [{}]", shorthand);
        let parent_options = vec![(parent_label, Some(parent_id))];

        let wizard =
            CategoryWizard::new_edit(existing_names, initial, parent_options, false, false);

        let mut interaction = MockInteraction::new(
            vec![
                PromptResponse::Value("Rent (Updated)".into()),
                PromptResponse::Value("2".into()),
                PromptResponse::Value("Updated note".into()),
            ],
            vec![ConfirmationResponse::Confirm],
        );

        let result = FormEngine::new(&wizard).run(&mut interaction).unwrap();
        match result {
            FormResult::Completed(data) => {
                assert_eq!(data.id, Some(category_id));
                assert_eq!(data.name, "Rent (Updated)");
                assert_eq!(data.kind, CategoryKind::Expense);
                assert!(!data.is_custom);
                assert_eq!(data.parent_id, Some(parent_id));
                assert_eq!(data.notes.as_deref(), Some("Updated note"));
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
}
