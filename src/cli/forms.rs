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
use dialoguer::{theme::ColorfulTheme, Input};
use uuid::Uuid;

use crate::cli::output;
use crate::ledger::{AccountKind, CategoryKind};

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
    Custom(Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>),
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
            Validator::Custom(func) => func(input)
                .map_err(|msg| ValidationError::new(msg))
                .map(|value| value),
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
        if trimmed.is_empty() {
            Ok(String::new())
        } else {
            trimmed
                .parse::<f64>()
                .map(|value| value.to_string())
                .map_err(|_| "Enter a numeric amount".into())
        }
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

        let mut fields = Vec::new();
        fields.push(FieldDescriptor::new(
            "name",
            "Account name",
            FieldKind::Text,
            name_validator,
        ));
        fields.push(FieldDescriptor::new(
            "kind",
            "Account type",
            FieldKind::Choice(kind_choices.options()),
            kind_validator,
        ));
        fields.push(
            FieldDescriptor::new(
                "category",
                "Linked category",
                FieldKind::Choice(category_choices.options()),
                category_validator,
            )
            .with_optional(),
        );
        fields.push(
            FieldDescriptor::new(
                "opening_balance",
                "Opening balance",
                FieldKind::Decimal,
                make_optional_decimal_validator(),
            )
            .with_optional(),
        );
        fields.push(
            FieldDescriptor::new("notes", "Notes", FieldKind::Text, make_notes_validator(512))
                .with_optional(),
        );

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
pub trait FormInteraction {
    fn prompt_field(
        &mut self,
        descriptor: &FieldDescriptor,
        default: Option<&str>,
    ) -> PromptResponse;

    fn confirm(&mut self, summary: &FormSummary) -> ConfirmationResponse;
}

/// Default interactive implementation backed by `dialoguer`.
pub struct DialoguerInteraction<'a> {
    theme: &'a ColorfulTheme,
}

impl<'a> DialoguerInteraction<'a> {
    pub fn new(theme: &'a ColorfulTheme) -> Self {
        Self { theme }
    }
}

impl<'a> FormInteraction for DialoguerInteraction<'a> {
    fn prompt_field(
        &mut self,
        _descriptor: &FieldDescriptor,
        default: Option<&str>,
    ) -> PromptResponse {
        let mut input = Input::<String>::with_theme(self.theme);
        if let Some(default_value) = default {
            if !default_value.is_empty() {
                input = input.with_initial_text(default_value);
            }
        }
        input = input.allow_empty(true).with_prompt("> ");
        let response = match input.interact_text() {
            Ok(value) => value,
            Err(_) => return PromptResponse::Cancel,
        };

        let trimmed = response.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "cancel" => PromptResponse::Cancel,
            "back" => PromptResponse::Back,
            "help" => PromptResponse::Help,
            _ => {
                if trimmed.is_empty() {
                    if default.is_some() {
                        PromptResponse::Keep
                    } else {
                        PromptResponse::Value(String::new())
                    }
                } else if default.map(|d| d == response).unwrap_or(false) {
                    PromptResponse::Keep
                } else {
                    PromptResponse::Value(response)
                }
            }
        }
    }

    fn confirm(&mut self, _summary: &FormSummary) -> ConfirmationResponse {
        loop {
            let input = Input::<String>::with_theme(self.theme)
                .with_prompt("Confirm? (yes/no/back/cancel)")
                .allow_empty(true);
            let response = match input.interact_text() {
                Ok(value) => value,
                Err(_) => return ConfirmationResponse::Cancel,
            };
            match response.trim().to_ascii_lowercase().as_str() {
                "" | "y" | "yes" => return ConfirmationResponse::Confirm,
                "n" | "no" | "cancel" => return ConfirmationResponse::Cancel,
                "back" => return ConfirmationResponse::Back,
                _ => {
                    output::warning("Enter yes, no, back, or cancel.");
                }
            }
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
                    output::warning("Already at the first field.");
                    Ok(FormSessionEvent::Repeat)
                }
            }
            PromptResponse::Help => {
                if let Some(help) = field.help {
                    output::info(help);
                } else {
                    output::info("No additional information available for this field.");
                }
                Ok(FormSessionEvent::Repeat)
            }
            PromptResponse::Keep => {
                if let Some(existing) = self.values.get(field.key) {
                    self.values.insert(field.key.to_string(), existing.clone());
                    self.index += 1;
                    Ok(FormSessionEvent::Moved)
                } else if field.required {
                    output::warning("This field is required.");
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
                    output::warning(&err.message);
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
                render_summary(&summary);
                match interaction.confirm(&summary) {
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

            render_prompt(step.descriptor, step.default.as_deref());
            let response = interaction.prompt_field(step.descriptor, step.default.as_deref());
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

fn render_prompt(descriptor: &FieldDescriptor, default: Option<&str>) {
    match default {
        Some(default_value) => output::info(format!("{} [{}]:", descriptor.label, default_value)),
        None => output::info(format!("{}:", descriptor.label)),
    }

    if let FieldKind::Choice(options) = &descriptor.kind {
        output::info("Options:");
        for option in options {
            output::info(format!("  {}", option));
        }
    }
}

fn render_summary(summary: &FormSummary) {
    output::info("Review your entries:");
    for (key, value) in &summary.entries {
        output::info(format!("  {}: {}", key, value));
    }
    output::info("Confirm to apply changes, or type back/cancel to modify.");
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
        fn prompt_field(
            &mut self,
            descriptor: &FieldDescriptor,
            _default: Option<&str>,
        ) -> PromptResponse {
            let response = self
                .prompts
                .pop_front()
                .unwrap_or(PromptResponse::Value("".into()));
            if matches!(response, PromptResponse::Help) {
                self.help_hits += 1;
                if descriptor.help.is_none() {
                    // Ensure we don't stall; return keep afterwards.
                    return PromptResponse::Keep;
                }
            }
            response
        }

        fn confirm(&mut self, _summary: &FormSummary) -> ConfirmationResponse {
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
                assert_eq!(data.is_custom, false);
                assert_eq!(data.parent_id, Some(parent_id));
                assert_eq!(data.notes.as_deref(), Some("Updated note"));
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
}
