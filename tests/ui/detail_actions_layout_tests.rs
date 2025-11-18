use std::sync::Once;

use budget_core::cli::output::{set_preferences, OutputPreferences};
use budget_core::cli::ui::detail_actions::{
    DetailAction, DetailActionResult, DetailActionsMenu,
};
use crossterm::event::KeyCode;

fn init_style() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_var("NO_COLOR", "1");
        set_preferences(OutputPreferences::default());
    });
}

#[test]
fn highlight_prefix_present_on_selected_item() {
    init_style();
    let menu = DetailActionsMenu::new(
        "Actions",
        vec![
            DetailAction::new("edit", "EDIT", "Edit entry"),
            DetailAction::new("delete", "DELETE", "Delete entry"),
        ],
    );
    let snapshot = menu.render_snapshot(0);
    assert!(
        snapshot.lines().any(|line| line.trim_start().starts_with(">")),
        "expected highlight marker:\n{snapshot}"
    );
}

#[test]
fn esc_returns_escaped() {
    init_style();
    let menu = DetailActionsMenu::new(
        "Actions",
        vec![DetailAction::new("edit", "EDIT", "Edit entry")],
    );
    assert_eq!(
        menu.run_simulated(&[KeyCode::Esc]),
        DetailActionResult::Escaped
    );
}

#[test]
fn enter_selects_current_action() {
    init_style();
    let actions = vec![DetailAction::new("edit", "EDIT", "Edit entry")];
    let menu = DetailActionsMenu::new("Actions", actions);
    match menu.run_simulated(&[KeyCode::Enter]) {
        DetailActionResult::Selected(action) => assert_eq!(action.id, "edit"),
        other => panic!("expected selection, got {:?}", other),
    }
}

#[test]
fn empty_actions_yield_empty_result() {
    init_style();
    let menu = DetailActionsMenu::new("Actions", vec![]);
    assert!(matches!(menu.run_simulated(&[]), DetailActionResult::Empty));
}
