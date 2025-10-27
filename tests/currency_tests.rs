use budget_core::currency::{format_currency_value, CurrencyCode, FormatOptions, LocaleConfig};

#[test]
fn formats_currency_with_locale() {
    let mut locale = LocaleConfig::default();
    locale.decimal_separator = ',';
    locale.grouping_separator = ' ';
    let options = FormatOptions {
        currency_display: budget_core::currency::CurrencyDisplay::Symbol,
        negative_style: budget_core::currency::NegativeStyle::Parentheses,
        screen_reader_mode: false,
        high_contrast_mode: false,
    };
    let code = CurrencyCode::new("EUR");
    let formatted = format_currency_value(-1234.5, &code, &locale, &options);
    assert_eq!(formatted, "€ (1 234,50)");
}
