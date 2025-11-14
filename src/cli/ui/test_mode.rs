use once_cell::sync::Lazy;
use std::{collections::VecDeque, env, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTestEvent {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Esc,
}

#[derive(Debug, Clone)]
pub enum TextTestInput {
    Value(String),
    Keep,
    Back,
    Help,
    Cancel,
    Escape,
}

static MENU_EVENTS: Lazy<Option<Mutex<VecDeque<Vec<MenuTestEvent>>>>> = Lazy::new(|| {
    env::var("BUFY_TEST_MENU_EVENTS").ok().map(|raw| {
        let sequences = raw
            .split('|')
            .filter_map(|segment| {
                let trimmed = segment.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let events = trimmed
                    .split(',')
                    .filter_map(|token| parse_menu_event(token.trim()))
                    .collect::<Vec<_>>();
                if events.is_empty() {
                    None
                } else {
                    Some(events)
                }
            })
            .collect::<VecDeque<_>>();
        Mutex::new(sequences)
    })
});

static TEXT_INPUTS: Lazy<Option<Mutex<VecDeque<TextTestInput>>>> = Lazy::new(|| {
    env::var("BUFY_TEST_TEXT_INPUTS").ok().map(|raw| {
        let entries = raw
            .split('|')
            .filter_map(|segment| {
                let trimmed = segment.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(parse_text_input(trimmed))
                }
            })
            .collect::<VecDeque<_>>();
        Mutex::new(entries)
    })
});

pub fn is_enabled() -> bool {
    MENU_EVENTS.is_some() || TEXT_INPUTS.is_some()
}

pub fn next_menu_events(label: &str) -> Option<Vec<MenuTestEvent>> {
    MENU_EVENTS.as_ref().map(|queue| {
        let mut guard = queue.lock().expect("menu event queue poisoned");
        guard.pop_front().unwrap_or_else(|| {
            panic!("BUFY_TEST_MENU_EVENTS exhausted before `{label}` menu rendered")
        })
    })
}

pub fn next_text_input(label: &str) -> Option<TextTestInput> {
    TEXT_INPUTS.as_ref().map(|queue| {
        let mut guard = queue.lock().expect("text input queue poisoned");
        guard
            .pop_front()
            .unwrap_or_else(|| panic!("BUFY_TEST_TEXT_INPUTS exhausted before prompt `{label}`"))
    })
}

fn parse_menu_event(token: &str) -> Option<MenuTestEvent> {
    if token.is_empty() {
        return None;
    }
    match token.to_ascii_uppercase().as_str() {
        "UP" => Some(MenuTestEvent::Up),
        "DOWN" => Some(MenuTestEvent::Down),
        "HOME" => Some(MenuTestEvent::Home),
        "END" => Some(MenuTestEvent::End),
        "PAGEUP" | "PAGE_UP" => Some(MenuTestEvent::PageUp),
        "PAGEDOWN" | "PAGE_DOWN" => Some(MenuTestEvent::PageDown),
        "ENTER" | "RETURN" => Some(MenuTestEvent::Enter),
        "ESC" | "ESCAPE" => Some(MenuTestEvent::Esc),
        _ => None,
    }
}

fn parse_text_input(token: &str) -> TextTestInput {
    match token.to_ascii_uppercase().as_str() {
        "<ESC>" | "ESC" => TextTestInput::Escape,
        "<CANCEL>" => TextTestInput::Cancel,
        "<BACK>" | "BACK" => TextTestInput::Back,
        "<HELP>" | "HELP" => TextTestInput::Help,
        "<KEEP>" | "KEEP" => TextTestInput::Keep,
        "<BLANK>" | "<EMPTY>" => TextTestInput::Value(String::new()),
        _ => TextTestInput::Value(token.to_string()),
    }
}
