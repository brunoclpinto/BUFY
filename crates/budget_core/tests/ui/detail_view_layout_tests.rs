use std::sync::Once;

use budget_core::cli::output::{set_preferences, OutputPreferences};
use budget_core::cli::ui::detail_view::DetailView;

fn init_style() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_var("NO_COLOR", "1");
        set_preferences(OutputPreferences::default());
    });
}

#[test]
fn detail_view_aligns_keys_and_values() {
    init_style();
    let view = DetailView::new("Render")
        .with_field("short", "\"A\"")
        .with_field("much_longer_key", "\"B\"");
    let output = view.render();
    let lines: Vec<&str> = output.lines().collect();
    let colon_positions: Vec<usize> = lines
        .iter()
        .filter(|line| line.contains(':') && line.trim_start().starts_with('"'))
        .map(|line| line.find(':').unwrap())
        .collect();
    assert!(
        colon_positions.windows(2).all(|pair| pair[0] == pair[1]),
        "colons misaligned:\n{output}"
    );
}

#[test]
fn detail_view_falls_back_to_dash_for_empty_values() {
    init_style();
    let view = DetailView::new("Render").with_field("empty", "");
    let output = view.render();
    assert!(
        output.contains("—"),
        "expected em dash for empty values:\n{output}"
    );
}

#[test]
fn horizontal_rules_at_least_40_chars() {
    init_style();
    let view = DetailView::new("Short").with_field("k", "v");
    let output = view.render();
    let top_rule = output.lines().nth(1).expect("rule line");
    assert!(
        top_rule.chars().all(|ch| ch == '─') && top_rule.len() >= 40,
        "rule should be >=40 chars: {top_rule}"
    );
}
