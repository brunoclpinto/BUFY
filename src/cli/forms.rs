//! Wizard-style form framework used by interactive CLI commands.
//!
//! Phase 14 defines a reusable contract for multi-step data entry. Concrete
//! entities (accounts, categories, transactions, …) will plug into this
//! framework in subsequent phases by describing their fields and leveraging the
//! generic form engine implemented here.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use chrono::{NaiveDate, NaiveTime};

use crate::cli::output;

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
        output::info(format!("Options: {}", options.join(", ")));
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
    use std::sync::OnceLock;

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
}
