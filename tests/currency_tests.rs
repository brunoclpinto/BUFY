use budget_core::{
    currency::{
        format_currency_value, CurrencyCode, FormatOptions, FxBook, FxRate, LocaleConfig,
        ValuationPolicy,
    },
    ledger::{Account, AccountKind, BudgetPeriod, Ledger, Transaction},
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

#[test]
fn valuation_policy_selects_expected_rate() {
    let mut ledger = Ledger::new("Valuation Policy", BudgetPeriod::default());
    ledger.base_currency = CurrencyCode::new("USD");

    let mut eur_account = Account::new("EUR Checking", AccountKind::Bank);
    eur_account.currency = Some("EUR".into());
    let eur_id = ledger.add_account(eur_account);

    let mut usd_account = Account::new("USD Rent", AccountKind::ExpenseDestination);
    usd_account.currency = Some("USD".into());
    let usd_id = ledger.add_account(usd_account);

    let txn_date = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
    ledger.fx_book.add_rate(FxRate {
        from: CurrencyCode::new("EUR"),
        to: CurrencyCode::new("USD"),
        date: txn_date,
        rate: 1.2,
        source: Some("ECB".into()),
        notes: None,
    });
    ledger.fx_book.add_rate(FxRate {
        from: CurrencyCode::new("EUR"),
        to: CurrencyCode::new("USD"),
        date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
        rate: 1.4,
        source: Some("ECB".into()),
        notes: None,
    });

    let txn = Transaction::new(eur_id, usd_id, None, txn_date, 100.0);
    ledger.add_transaction(txn);

    ledger.valuation_policy = ValuationPolicy::TransactionDate;
    let txn_summary = ledger.summarize_period_containing(txn_date);
    assert!(
        (txn_summary.totals.budgeted - 120.0).abs() < f64::EPSILON,
        "transaction-date policy should use the 1.2 rate"
    );

    ledger.valuation_policy = ValuationPolicy::ReportDate;
    let report_summary = ledger.summarize_period_containing(txn_date);
    assert!(
        (report_summary.totals.budgeted - 140.0).abs() < f64::EPSILON,
        "report-date policy should use the month-end rate"
    );
}
