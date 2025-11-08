use budget_core::ledger::{
    account::{Account, AccountKind},
    category::{Category, CategoryKind},
    BudgetPeriod, Ledger,
};
use budget_core::storage::json_backend::{
    load_ledger_from_path as load_ledger_from_file, save_ledger_to_path as save_ledger_to_file,
};
use chrono::{Duration, NaiveDate};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use tempfile::tempdir;

fn build_sample_ledger(txn_count: usize) -> Ledger {
    let mut ledger = Ledger::new("Benchmark", BudgetPeriod::default());

    let checking = ledger.add_account(Account::new("Checking", AccountKind::Bank));
    let savings = ledger.add_account(Account::new("Savings", AccountKind::Savings));
    let groceries = ledger.add_category(Category::new("Groceries", CategoryKind::Expense));

    let start_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

    for idx in 0..txn_count {
        let scheduled = start_date + Duration::days((idx % 365) as i64);
        let mut txn = budget_core::ledger::transaction::Transaction::new(
            checking,
            savings,
            Some(groceries),
            scheduled,
            50.0 + (idx % 100) as f64,
        );
        if idx % 3 == 0 {
            txn.actual_date = Some(scheduled + Duration::days(1));
            txn.actual_amount = Some(txn.budgeted_amount * 0.95);
        }
        ledger.add_transaction(txn);
    }

    ledger.refresh_recurrence_metadata();
    ledger
}

fn bench_ledger_io(c: &mut Criterion) {
    let ledger = build_sample_ledger(black_box(10_000));
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("ledger.json");

    c.bench_function("ledger_save_10k", |b| {
        b.iter(|| {
            save_ledger_to_file(&ledger, &file_path).expect("save ledger");
        })
    });

    save_ledger_to_file(&ledger, &file_path).expect("seed");

    c.bench_function("ledger_load_10k", |b| {
        b.iter(|| {
            let loaded = load_ledger_from_file(&file_path).expect("load ledger");
            black_box(loaded);
        })
    });
}

fn bench_ledger_summaries(c: &mut Criterion) {
    let ledger = build_sample_ledger(black_box(10_000));
    let reference = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();

    c.bench_function("budget_summary_current", |b| {
        b.iter(|| {
            let summary = ledger.summarize_period_containing(reference);
            black_box(summary);
        })
    });

    c.bench_function("forecast_window_report", |b| {
        b.iter_batched(
            || ledger.clone(),
            |ledger_clone| {
                let window = ledger_clone.budget_window_for(reference);
                let report = ledger_clone
                    .forecast_window_report(window, reference, None)
                    .expect("forecast");
                black_box(report);
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_ledger_io, bench_ledger_summaries);
criterion_main!(benches);
