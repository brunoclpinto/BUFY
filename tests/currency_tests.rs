use budget_core::currency::{
    format_currency_value, CurrencyCode, FormatOptions, FxBook, FxRate, LocaleConfig,
};
use chrono::NaiveDate;

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

#[test]
fn fx_lookup_uses_nearest_prior() {
    let mut book = FxBook::new();
    book.tolerance.days = 3;
    book.add_rate(FxRate {
        from: CurrencyCode::new("EUR"),
        to: CurrencyCode::new("USD"),
        date: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        rate: 1.1,
        source: Some("ECB".into()),
        notes: None,
    });
    let lookup = book
        .lookup_rate("EUR", "USD", NaiveDate::from_ymd_opt(2025, 1, 12).unwrap())
        .expect("rate");
    assert!((lookup.rate - 1.1).abs() < f64::EPSILON);
    assert_eq!(lookup.date, NaiveDate::from_ymd_opt(2025, 1, 10).unwrap());
}
