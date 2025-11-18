use crossterm::event::{self, Event, KeyCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Char(char),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscBehavior {
    Back,
    Cancel,
    Ignore,
}

pub fn read_nav_key() -> NavKey {
    match event::read() {
        Ok(Event::Key(key)) => match key.code {
            KeyCode::Up => NavKey::Up,
            KeyCode::Down => NavKey::Down,
            KeyCode::Left => NavKey::Left,
            KeyCode::Right => NavKey::Right,
            KeyCode::Enter => NavKey::Enter,
            KeyCode::Esc => NavKey::Esc,
            KeyCode::Char(c) => NavKey::Char(c),
            _ => NavKey::Unknown,
        },
        _ => NavKey::Unknown,
    }
}

pub fn navigation_loop<F>(mut render: F) -> NavKey
where
    F: FnMut(),
{
    loop {
        render();
        match read_nav_key() {
            NavKey::Unknown => continue,
            key => return key,
        }
    }
}
