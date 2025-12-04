use crate::cli::{io, ui::style::UiStyle};

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
    pub fn render(menu: &Menu, style: &UiStyle) {
        let prefix = if style.use_icons { "📂 " } else { "" };
        let heading = format!("{prefix}{}", menu.title);
        let _ = io::println_text(&style.apply_header_style(&heading));
        if !style.plain_mode {
            let _ = io::println_text(&style.horizontal_line(menu.title.len().max(4)));
        }
        for item in &menu.items {
            let marker = if style.use_icons {
                if item.enabled {
                    "  •"
                } else {
                    "  ×"
                }
            } else if item.enabled {
                "  *"
            } else {
                "  -"
            };
            match &item.description {
                Some(description) => {
                    let line = format!("{marker} {:<16} {description}", item.label);
                    let _ = io::println_text(&line);
                }
                None => {
                    let line = format!("{marker} {}", item.label);
                    let _ = io::println_text(&line);
                }
            }
        }
    }
}
