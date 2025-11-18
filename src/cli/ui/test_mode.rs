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

struct MenuQueue {
    enabled: bool,
    events: VecDeque<Vec<MenuTestEvent>>,
}

impl MenuQueue {
    fn from_env() -> Self {
        if let Ok(raw) = env::var("BUFY_TEST_MENU_EVENTS") {
            Self {
                enabled: true,
                events: parse_menu_sequences(&raw),
            }
        } else {
            Self::new()
        }
    }

    fn new() -> Self {
        Self {
            enabled: false,
            events: VecDeque::new(),
        }
    }
}

struct TextQueue {
    enabled: bool,
    inputs: VecDeque<TextTestInput>,
}

impl TextQueue {
    fn from_env() -> Self {
        if let Ok(raw) = env::var("BUFY_TEST_TEXT_INPUTS") {
            Self {
                enabled: true,
                inputs: parse_text_sequences(&raw),
            }
        } else {
            Self::new()
        }
    }

    fn new() -> Self {
        Self {
            enabled: false,
            inputs: VecDeque::new(),
        }
    }
}

static MENU_EVENTS: Lazy<Mutex<MenuQueue>> = Lazy::new(|| Mutex::new(MenuQueue::from_env()));

static TEXT_INPUTS: Lazy<Mutex<TextQueue>> = Lazy::new(|| Mutex::new(TextQueue::from_env()));

pub fn is_enabled() -> bool {
    MENU_EVENTS
        .lock()
        .expect("menu event queue poisoned")
        .enabled
        || TEXT_INPUTS
            .lock()
            .expect("text input queue poisoned")
            .enabled
}

pub fn next_menu_events(label: &str) -> Option<Vec<MenuTestEvent>> {
    let mut guard = MENU_EVENTS.lock().expect("menu event queue poisoned");
    if !guard.enabled {
        return None;
    }
    Some(
        guard
            .events
            .pop_front()
            .unwrap_or_else(|| panic!("Menu events exhausted before `{label}` menu rendered")),
    )
}

pub fn next_text_input(label: &str) -> Option<TextTestInput> {
    let mut guard = TEXT_INPUTS.lock().expect("text input queue poisoned");
    if !guard.enabled {
        return None;
    }
    Some(
        guard
            .inputs
            .pop_front()
            .unwrap_or_else(|| panic!("Text inputs exhausted before prompt `{label}`")),
    )
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

fn parse_menu_sequences(raw: &str) -> VecDeque<Vec<MenuTestEvent>> {
    raw.split('|')
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
        .collect()
}

fn parse_text_sequences(raw: &str) -> VecDeque<TextTestInput> {
    raw.split('|')
        .filter_map(|segment| {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(parse_text_input(trimmed))
            }
        })
        .collect()
}

pub fn install_menu_events(events: Vec<Vec<MenuTestEvent>>) {
    let mut guard = MENU_EVENTS.lock().expect("menu event queue poisoned");
    guard.enabled = true;
    guard.events = events.into();
}

pub fn reset_menu_events() {
    let mut guard = MENU_EVENTS.lock().expect("menu event queue poisoned");
    guard.enabled = false;
    guard.events.clear();
}

pub fn install_text_inputs(inputs: Vec<TextTestInput>) {
    let mut guard = TEXT_INPUTS.lock().expect("text input queue poisoned");
    guard.enabled = true;
    guard.inputs = inputs.into();
}

pub fn reset_text_inputs() {
    let mut guard = TEXT_INPUTS.lock().expect("text input queue poisoned");
    guard.enabled = false;
    guard.inputs.clear();
}
