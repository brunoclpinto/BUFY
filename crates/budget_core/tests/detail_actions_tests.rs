use budget_core::cli::ui::detail_actions::{DetailAction, DetailActionResult, DetailActionsMenu};
use crossterm::event::KeyCode;

fn sample_actions() -> Vec<DetailAction> {
    vec![
        DetailAction::new("edit", "EDIT", "Edit item"),
        DetailAction::new("delete", "DELETE", "Delete item"),
        DetailAction::new("complete", "COMPLETE", "Mark as complete"),
    ]
}

#[test]
fn empty_actions_yield_empty_result() {
    let menu = DetailActionsMenu::new("Actions", vec![]);
    assert!(matches!(menu.run_simulated(&[]), DetailActionResult::Empty));
}

#[test]
fn selects_single_action_with_enter() {
    let actions = vec![DetailAction::new("edit", "EDIT", "Edit item")];
    let menu = DetailActionsMenu::new("Actions", actions);
    let result = menu.run_simulated(&[KeyCode::Enter]);
    match result {
        DetailActionResult::Selected(action) => assert_eq!(action.id, "edit"),
        other => panic!("Unexpected result: {:?}", other),
    }
}

#[test]
fn navigates_down_before_selecting() {
    let menu = DetailActionsMenu::new("Actions", sample_actions());
    let result = menu.run_simulated(&[KeyCode::Down, KeyCode::Enter]);
    match result {
        DetailActionResult::Selected(action) => assert_eq!(action.id, "delete"),
        other => panic!("Unexpected result: {:?}", other),
    }
}

#[test]
fn wrap_around_with_up_from_start() {
    let menu = DetailActionsMenu::new("Actions", sample_actions());
    let result = menu.run_simulated(&[KeyCode::Up, KeyCode::Enter]);
    match result {
        DetailActionResult::Selected(action) => assert_eq!(action.id, "complete"),
        other => panic!("Unexpected result: {:?}", other),
    }
}

#[test]
fn escape_cancels_selection() {
    let menu = DetailActionsMenu::new("Actions", sample_actions());
    assert!(matches!(
        menu.run_simulated(&[KeyCode::Esc]),
        DetailActionResult::Escaped
    ));
}
