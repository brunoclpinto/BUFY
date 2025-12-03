use budget_core::cli::ui::prompts::{
    choice_menu, text_input, ChoicePromptResult, TextPromptResult,
};
use budget_core::cli::ui::test_mode::{
    install_menu_events, install_text_inputs, reset_menu_events, reset_text_inputs,
    MenuTestEvent, TextTestInput,
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

struct TextGuard;

impl TextGuard {
    fn with_inputs(inputs: Vec<TextTestInput>) -> Self {
        install_text_inputs(inputs);
        Self
    }
}

impl Drop for TextGuard {
    fn drop(&mut self) {
        reset_text_inputs();
    }
}

#[test]
fn choice_menu_selects_requested_option() {
    let _guard = MenuGuard::with_events(vec![vec![MenuTestEvent::Down, MenuTestEvent::Enter]]);
    let options = vec!["Alpha".into(), "Beta".into()];
    let result = choice_menu("Choose option", &[], &options, None, false)
        .expect("menu selection should succeed");
    match result {
        ChoicePromptResult::Value(value) => assert_eq!(value, "Beta"),
        other => panic!("Expected direct selection, got {:?}", other),
    }
}

#[test]
fn choice_menu_supports_back_option() {
    let _guard = MenuGuard::with_events(vec![vec![
        MenuTestEvent::Down,
        MenuTestEvent::Down,
        MenuTestEvent::Enter,
    ]]);
    let options = vec!["Alpha".into(), "Beta".into()];
    let result = choice_menu("Choose option", &[], &options, None, true)
        .expect("menu selection should succeed");
    assert!(matches!(result, ChoicePromptResult::Back));
}

#[test]
fn text_input_returns_scripted_value() {
    let _guard = TextGuard::with_inputs(vec![TextTestInput::Value("Demo".into())]);
    match text_input("Name", None).expect("text input should succeed") {
        TextPromptResult::Value(value) => assert_eq!(value, "Demo"),
        other => panic!("Expected scripted value, got {:?}", other),
    }
}

#[test]
fn text_input_handles_escape_signal() {
    let _guard = TextGuard::with_inputs(vec![TextTestInput::Escape]);
    assert!(matches!(
        text_input("Name", None).expect("text input should succeed"),
        TextPromptResult::Escape
    ));
}
