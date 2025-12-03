use budget_core::cli::ui::navigation::{map_key_code, NavKey};
use crossterm::event::KeyCode;

#[test]
fn arrow_keys_map_to_navigation_variants() {
    assert_eq!(map_key_code(KeyCode::Up), NavKey::Up);
    assert_eq!(map_key_code(KeyCode::Down), NavKey::Down);
    assert_eq!(map_key_code(KeyCode::Left), NavKey::Left);
    assert_eq!(map_key_code(KeyCode::Right), NavKey::Right);
}

#[test]
fn enter_and_escape_are_identified() {
    assert_eq!(map_key_code(KeyCode::Enter), NavKey::Enter);
    assert_eq!(map_key_code(KeyCode::Esc), NavKey::Esc);
}

#[test]
fn character_keys_propagate_value() {
    assert_eq!(map_key_code(KeyCode::Char('a')), NavKey::Char('a'));
    assert_eq!(map_key_code(KeyCode::Char('Z')), NavKey::Char('Z'));
}

#[test]
fn unsupported_keys_return_unknown() {
    assert_eq!(map_key_code(KeyCode::Backspace), NavKey::Unknown);
}
