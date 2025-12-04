use crate::cli::io;

/// Represents a single menu entry.
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub description: Option<String>,
    pub enabled: bool,
}

/// Declarative menu configuration.
#[derive(Debug, Clone)]
pub struct Menu {
    pub title: String,
    pub items: Vec<MenuItem>,
}

impl Menu {
    pub fn new<T: Into<String>>(title: T) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
        }
    }

    pub fn add_item<T: Into<String>>(&mut self, label: T, description: Option<T>, enabled: bool) {
        self.items.push(MenuItem {
            label: label.into(),
            description: description.map(|value| value.into()),
            enabled,
        });
    }
}

/// Renders simple bullet-style menus.
pub struct MenuRenderer;

impl MenuRenderer {
    pub fn render(menu: &Menu) {
        let underline = "─".repeat(menu.title.len().max(4));
        let _ = io::println_text(&menu.title);
        let _ = io::println_text(&underline);
        for item in &menu.items {
            let prefix = if item.enabled { "  •" } else { "  ×" };
            match &item.description {
                Some(description) => {
                    let line = format!("{prefix} {:<16} {description}", item.label);
                    let _ = io::println_text(&line);
                }
                None => {
                    let line = format!("{prefix} {}", item.label);
                    let _ = io::println_text(&line);
                }
            }
        }
    }
}
