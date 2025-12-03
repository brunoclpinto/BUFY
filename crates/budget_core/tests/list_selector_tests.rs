use budget_core::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use budget_core::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crossterm::event::KeyCode;

fn sample_table() -> Table {
    Table {
        show_headers: true,
        padding: 1,
        columns: vec![
            TableColumn {
                header: "NAME".into(),
                min_width: 4,
                max_width: None,
                alignment: Alignment::Left,
            },
            TableColumn {
                header: "VALUE".into(),
                min_width: 5,
                max_width: None,
                alignment: Alignment::Right,
            },
        ],
        rows: vec![
            vec!["One".into(), "1".into()],
            vec!["Two".into(), "2".into()],
            vec!["Three".into(), "3".into()],
        ],
    }
}

#[test]
fn empty_table_returns_empty() {
    let table = Table {
        rows: Vec::new(),
        ..sample_table()
    };
    let selector = ListSelector::new(&table);
    assert!(matches!(
        selector.run_simulated(&[]),
        ListSelectionResult::Empty
    ));
}

#[test]
fn default_selection_is_first_row() {
    let table = sample_table();
    let selector = ListSelector::new(&table);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Enter]),
        ListSelectionResult::Selected(0)
    );
}

#[test]
fn up_arrow_wraps_to_end() {
    let table = sample_table();
    let selector = ListSelector::new(&table);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Up, KeyCode::Enter]),
        ListSelectionResult::Selected(2)
    );
}

#[test]
fn down_arrow_cycles_forward() {
    let table = sample_table();
    let selector = ListSelector::new(&table);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Down, KeyCode::Down, KeyCode::Enter]),
        ListSelectionResult::Selected(2)
    );
}

#[test]
fn escape_aborts_selection() {
    let table = sample_table();
    let selector = ListSelector::new(&table);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Down, KeyCode::Esc]),
        ListSelectionResult::Escaped
    );
}
