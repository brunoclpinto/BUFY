use budget_core::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use budget_core::cli::ui::list_selector::{ListSelectionResult, ListSelector};
use budget_core::cli::ui::table_renderer::{Alignment, Table, TableColumn};
use crossterm::event::KeyCode;

fn sample_table() -> Table {
    Table {
        show_headers: true,
        padding: 1,
        columns: vec![TableColumn {
            header: "NAME".into(),
            min_width: 4,
            max_width: None,
            alignment: Alignment::Left,
        }],
        rows: vec![vec!["A".into()], vec!["B".into()], vec!["C".into()]],
    }
}

#[test]
fn list_selector_wraps_and_escapes() {
    let table = sample_table();
    let selector = ListSelector::new(&table);
    assert_eq!(
        selector.run_simulated(&[KeyCode::Up, KeyCode::Enter]),
        ListSelectionResult::Selected(2)
    );
    assert_eq!(
        selector.run_simulated(&[KeyCode::Esc]),
        ListSelectionResult::Escaped
    );
}

#[test]
fn detail_actions_menu_respects_navigation() {
    let actions = vec![
        DetailAction::new("one", "ONE", "First"),
        DetailAction::new("two", "TWO", "Second"),
    ];
    let menu = DetailActionsMenu::new("Actions", actions);
    assert!(matches!(
        menu.run_simulated(&[KeyCode::Down, KeyCode::Enter]),
        DetailActionResult::Selected(action) if action.id == "two"
    ));
    assert_eq!(
        menu.run_simulated(&[KeyCode::Esc]),
        DetailActionResult::Escaped
    );
}
