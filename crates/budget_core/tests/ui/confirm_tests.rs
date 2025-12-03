use budget_core::cli::ui::prompts::{confirm_menu, ConfirmationPromptResult};
use budget_core::cli::ui::test_mode::{
    install_menu_events, reset_menu_events, MenuTestEvent,
};

struct MenuGuard;

impl MenuGuard {
    fn with_events(events: Vec<Vec<MenuTestEvent>>) -> Self {
        install_menu_events(events);
        Self
    }
}

impl Drop for MenuGuard {
    fn drop(&mut self) {
        reset_menu_events();
    }
}

fn sample_context() -> Vec<String> {
    vec!["Review changes".into(), "Confirm to continue.".into()]
}

#[test]
fn confirm_menu_accepts_confirmation() {
    let _guard = MenuGuard::with_events(vec![vec![MenuTestEvent::Enter]]);
    let result = confirm_menu(&sample_context()).expect("menu result");
    assert_eq!(result, ConfirmationPromptResult::Confirm);
}

#[test]
fn confirm_menu_can_go_back() {
    let _guard = MenuGuard::with_events(vec![vec![
        MenuTestEvent::Down,
        MenuTestEvent::Enter,
    ]]);
    let result = confirm_menu(&sample_context()).expect("menu result");
    assert_eq!(result, ConfirmationPromptResult::Back);
}

#[test]
fn confirm_menu_escape_cancels() {
    let _guard = MenuGuard::with_events(vec![vec![MenuTestEvent::Esc]]);
    let result = confirm_menu(&sample_context()).expect("menu result");
    assert_eq!(result, ConfirmationPromptResult::Cancel);
}
