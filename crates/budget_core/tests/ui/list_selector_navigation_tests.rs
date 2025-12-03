use std::sync::Once;

use budget_core::cli::output::{set_preferences, OutputPreferences};
use budget_core::cli::ui::{
    list_selector::{ListSelectionResult, ListSelector},
    table_renderer::{Alignment, Table, TableColumn},
};
use crossterm::event::KeyCode;

fn init_style() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_var("NO_COLOR", "1");
        set_preferences(OutputPreferences::default());
    });
}

fn selector_with_rows(rows: Vec<Vec<&str>>) -> ListSelector<'static> {
    init_style();
    let table = Table {
        columns: vec![TableColumn {
            header: "NAME".into(),
            min_width: 4,
            max_width: None,
            alignment: Alignment::Left,
        }],
        rows: rows
            .into_iter()
            .map(|row| row.into_iter().map(|cell| cell.to_string()).collect())
            .collect(),
        show_headers: true,
        padding: 1,
    };
    let leaked = Box::leak(Box::new(table));
    ListSelector::new(leaked)
}

#[test]
fn single_row_selects_immediately() {
    let selector = selector_with_rows(vec![vec!["Alpha"]]);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Enter]),
        ListSelectionResult::Selected(0)
    );
}

#[test]
fn up_arrow_wraps_from_start() {
    let selector = selector_with_rows(vec![vec!["Alpha"], vec!["Beta"]]);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Up, KeyCode::Enter]),
        ListSelectionResult::Selected(1)
    );
}

#[test]
fn down_arrow_wraps_to_start() {
    let selector = selector_with_rows(vec![vec!["Alpha"], vec!["Beta"]]);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Down, KeyCode::Down, KeyCode::Enter]),
        ListSelectionResult::Selected(0)
    );
}

#[test]
fn escape_returns_escaped() {
    let selector = selector_with_rows(vec![vec!["Alpha"]]);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Esc]),
        ListSelectionResult::Escaped
    );
}

#[test]
fn handles_long_values_without_crash() {
    let selector = selector_with_rows(vec![vec!["This is a very long value that should be truncated"]]);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Enter]),
        ListSelectionResult::Selected(0)
    );
}
