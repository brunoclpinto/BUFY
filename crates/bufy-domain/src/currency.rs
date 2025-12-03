use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// ISO 4217 currency representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CurrencyCode(pub String);

impl CurrencyCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into().to_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CurrencyCode {
    fn default() -> Self {
        Self::new("USD")
    }
}

/// Locale-aware formatting preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleConfig {
    pub language_tag: String,
    pub decimal_separator: char,
    pub grouping_separator: char,
    pub date_format: DateFormatStyle,
    pub first_weekday: Weekday,
}

impl Default for LocaleConfig {
    fn default() -> Self {
        Self {
            language_tag: "en-US".into(),
            decimal_separator: '.',
            grouping_separator: ',',
            date_format: DateFormatStyle::Medium,
            first_weekday: Weekday::Mon,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FormatOptions {
    pub currency_display: CurrencyDisplay,
    pub negative_style: NegativeStyle,
    pub screen_reader_mode: bool,
    pub high_contrast_mode: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            currency_display: CurrencyDisplay::Symbol,
            negative_style: NegativeStyle::Sign,
            screen_reader_mode: false,
            high_contrast_mode: false,
        }
    }
}

impl FormatOptions {
    pub fn is_default(value: &FormatOptions) -> bool {
        value == &Self::default()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NegativeStyle {
    Sign,
    Parentheses,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CurrencyDisplay {
    Symbol,
    Code,
    SymbolAndCode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DateFormatStyle {
    Short,
    Medium,
    Long,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ValuationPolicy {
    #[default]
    TransactionDate,
    ReportDate,
    CustomDate(NaiveDate),
}

/// Results of a currency conversion for disclosure.
#[derive(Debug, Clone)]
pub struct ConvertedAmount {
    pub amount: f64,
    pub rate_used: f64,
    pub rate_date: NaiveDate,
    pub source: String,
    pub from: CurrencyCode,
    pub to: CurrencyCode,
}

impl ConvertedAmount {
    pub fn disclosure(&self) -> String {
        format!(
            "{} {} @ {} ({})",
            self.from.as_str(),
            self.rate_used,
            self.rate_date,
            self.source
        )
    }
}

/// Determines which date should be used for conversion.
pub fn policy_date(
    policy: &ValuationPolicy,
    transaction_date: NaiveDate,
    report_date: NaiveDate,
) -> NaiveDate {
    match policy {
        ValuationPolicy::TransactionDate => transaction_date,
        ValuationPolicy::ReportDate => report_date,
        ValuationPolicy::CustomDate(date) => *date,
    }
}

pub fn symbol_for(code: &str) -> String {
    match code {
        "USD" => "$".into(),
        "EUR" => "€".into(),
        "GBP" => "£".into(),
        "JPY" => "¥".into(),
        "CAD" => "CAD".into(),
        "AUD" => "A$".into(),
        "CHF" => "CHF".into(),
        _ => code.into(),
    }
}

pub fn minor_units_for(code: &str) -> u8 {
    match code {
        "JPY" => 0,
        "KWD" | "BHD" => 3,
        _ => 2,
    }
}

pub fn format_number(locale: &LocaleConfig, value: f64, precision: u8) -> String {
    let mut body = format!("{:.*}", precision as usize, value);
    if locale.decimal_separator != '.' {
        if let Some(pos) = body.find('.') {
            body.replace_range(pos..=pos, &locale.decimal_separator.to_string());
        }
    }
    if let Some(pos) = body.find(locale.decimal_separator) {
        let mut int_part = body[..pos].to_string();
        insert_grouping(&mut int_part, locale.grouping_separator);
        body = format!("{}{}", int_part, &body[pos..]);
    } else {
        insert_grouping(&mut body, locale.grouping_separator);
    }
    body
}

fn insert_grouping(int_part: &mut String, separator: char) {
    let mut cleaned = int_part.replace(separator, "");
    if cleaned.starts_with('-') {
        let sign = cleaned.remove(0);
        let grouped = group_digits(&cleaned, separator);
        *int_part = format!("{}{}", sign, grouped);
    } else {
        *int_part = group_digits(&cleaned, separator);
    }
}

fn group_digits(digits: &str, separator: char) -> String {
    let mut grouped = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx != 0 && idx % 3 == 0 {
            grouped.insert(0, separator);
        }
        grouped.insert(0, ch);
    }
    grouped
}

pub fn format_currency_value_with_precision(
    amount: f64,
    code: &CurrencyCode,
    locale: &LocaleConfig,
    options: &FormatOptions,
    precision_override: Option<u8>,
) -> String {
    let precision = precision_override.unwrap_or_else(|| minor_units_for(code.as_str()));
    let abs_value = amount.abs();
    let mut body = format_number(locale, abs_value, precision);
    if amount < 0.0 {
        body = match options.negative_style {
            NegativeStyle::Sign => format!("-{}", body),
            NegativeStyle::Parentheses => format!("({})", body),
        };
    }
    let symbol = symbol_for(code.as_str());
    let mut rendered_body = body.clone();
    if rendered_body.starts_with('(') {
        rendered_body = format!(" {}", body);
    }
    let formatted = match options.currency_display {
        CurrencyDisplay::Symbol => format!("{}{}", symbol, rendered_body),
        CurrencyDisplay::Code => format!("{} {}", code.as_str(), body),
        CurrencyDisplay::SymbolAndCode => {
            format!("{} {} ({})", symbol, rendered_body, code.as_str())
        }
    };
    if options.screen_reader_mode {
        if amount < 0.0 {
            format!(
                "minus {} {}",
                code.as_str(),
                formatted
                    .trim_start_matches('-')
                    .trim_matches(|c| c == '(' || c == ')')
            )
        } else {
            format!("{} {}", code.as_str(), formatted)
        }
    } else {
        formatted
    }
}

pub fn format_currency_value(
    amount: f64,
    code: &CurrencyCode,
    locale: &LocaleConfig,
    options: &FormatOptions,
) -> String {
    format_currency_value_with_precision(amount, code, locale, options, None)
}

pub fn format_date(locale: &LocaleConfig, date: NaiveDate) -> String {
    match locale.date_format {
        DateFormatStyle::Short => date.format("%Y-%m-%d").to_string(),
        DateFormatStyle::Medium => format!(
            "{:02} {} {}",
            date.day(),
            month_label(date.month()),
            date.year()
        ),
        DateFormatStyle::Long => format!(
            "{} {}, {}",
            date.weekday(),
            month_label(date.month()),
            date.year()
        ),
    }
}

fn month_label(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}
