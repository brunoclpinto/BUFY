use bufy_domain::currency::{format_currency_value, CurrencyCode, CurrencyDisplay, FormatOptions, LocaleConfig, NegativeStyle};

#[test]
fn formats_currency_with_locale() {
    let locale = LocaleConfig {
        decimal_separator: ',',
        grouping_separator: ' ',
        ..LocaleConfig::default()
    };
    let options = FormatOptions {
        currency_display: CurrencyDisplay::Symbol,
        negative_style: NegativeStyle::Parentheses,
        screen_reader_mode: false,
        high_contrast_mode: false,
    };
    let code = CurrencyCode::new("EUR");
    let formatted = format_currency_value(-1234.5, &code, &locale, &options);
    assert_eq!(formatted, "€ (1 234,50)");
}
