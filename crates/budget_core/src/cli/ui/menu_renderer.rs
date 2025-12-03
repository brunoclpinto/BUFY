use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::Stylize,
    terminal::{self, ClearType},
    ExecutableCommand,
};

use crate::cli::{
    io::write_line,
    ui::{
        style::{format_header, style},
        table_renderer::visible_width,
        test_mode::{self, MenuTestEvent},
    },
};

const DEFAULT_HINT: &str = "Use ↑ ↓ to navigate, Enter to select, ESC to return.";

#[derive(Clone, Debug)]
pub struct MenuUI {
    pub title: String,
    pub context: Option<String>,
    pub items: Vec<MenuUIItem>,
    pub initial_index: Option<usize>,
    pub footer_hint: Option<String>,
}

impl MenuUI {
    pub fn new(title: impl Into<String>, items: Vec<MenuUIItem>) -> Self {
        Self {
            title: title.into(),
            context: None,
            items,
            initial_index: None,
            footer_hint: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_initial_index(mut self, index: usize) -> Self {
        self.initial_index = Some(index);
        self
    }

    pub fn with_footer_hint(mut self, hint: impl Into<String>) -> Self {
        self.footer_hint = Some(hint.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct MenuUIItem {
    pub key: String,
    pub label: String,
    pub description: String,
    pub enabled: bool,
}

impl MenuUIItem {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            enabled: true,
        }
    }

    pub fn disabled(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            enabled: false,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Debug)]
pub enum MenuRenderError {
    Interrupted,
    EndOfInput,
    Io(io::Error),
}

impl From<io::Error> for MenuRenderError {
    fn from(err: io::Error) -> Self {
        MenuRenderError::Io(err)
    }
}

pub struct MenuRenderer;

impl MenuRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&self, menu: &MenuUI) -> Result<Option<String>, MenuRenderError> {
        if menu.items.is_empty() {
            return Ok(None);
        }

        if let Some(events) = test_mode::next_menu_events(&menu.title) {
            return self.show_with_script(menu, events);
        }

        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout.execute(cursor::Hide)?;

        let mut selected_index = Self::initial_index(menu);
        let result = loop {
            self.draw_frame(&mut stdout, menu, selected_index)?;
            let event = event::read()?;
            match event {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                break Err(MenuRenderError::Interrupted)
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                break Err(MenuRenderError::EndOfInput)
                            }
                            _ => continue,
                        }
                    }
                    match key.code {
                        KeyCode::Up => {
                            selected_index =
                                Self::previous_enabled_index(&menu.items, selected_index);
                        }
                        KeyCode::Down => {
                            selected_index = Self::next_enabled_index(&menu.items, selected_index);
                        }
                        KeyCode::Home => {
                            if let Some(idx) = Self::first_enabled_index(&menu.items) {
                                selected_index = idx;
                            }
                        }
                        KeyCode::End => {
                            if let Some(idx) = Self::last_enabled_index(&menu.items) {
                                selected_index = idx;
                            }
                        }
                        KeyCode::PageUp => {
                            selected_index = Self::page_up_index(&menu.items, selected_index);
                        }
                        KeyCode::PageDown => {
                            selected_index = Self::page_down_index(&menu.items, selected_index);
                        }
                        KeyCode::Enter => {
                            if menu.items[selected_index].enabled {
                                let key = menu.items[selected_index].key.clone();
                                break Ok(Some(key));
                            }
                        }
                        KeyCode::Esc => break Ok(None),
                        _ => {}
                    }
                }
                Event::Resize(_, _) => continue,
                Event::Mouse(_) => continue,
                Event::FocusGained | Event::FocusLost | Event::Paste(_) => continue,
            }
        };

        let clear_status = self.clear_screen(&mut stdout);
        stdout.execute(cursor::Show).ok();
        terminal::disable_raw_mode().ok();
        clear_status?;

        result
    }

    fn show_with_script(
        &self,
        menu: &MenuUI,
        events: Vec<MenuTestEvent>,
    ) -> Result<Option<String>, MenuRenderError> {
        if menu.items.is_empty() {
            return Ok(None);
        }
        let mut selected_index = Self::initial_index(menu);
        for event in events {
            match event {
                MenuTestEvent::Up => {
                    selected_index = Self::previous_enabled_index(&menu.items, selected_index);
                }
                MenuTestEvent::Down => {
                    selected_index = Self::next_enabled_index(&menu.items, selected_index);
                }
                MenuTestEvent::Home => {
                    if let Some(idx) = Self::first_enabled_index(&menu.items) {
                        selected_index = idx;
                    }
                }
                MenuTestEvent::End => {
                    if let Some(idx) = Self::last_enabled_index(&menu.items) {
                        selected_index = idx;
                    }
                }
                MenuTestEvent::PageUp => {
                    selected_index = Self::page_up_index(&menu.items, selected_index);
                }
                MenuTestEvent::PageDown => {
                    selected_index = Self::page_down_index(&menu.items, selected_index);
                }
                MenuTestEvent::Enter => {
                    if menu.items[selected_index].enabled {
                        self.print_snapshot(menu, selected_index);
                        return Ok(Some(menu.items[selected_index].key.clone()));
                    }
                }
                MenuTestEvent::Esc => {
                    self.print_snapshot(menu, selected_index);
                    return Ok(None);
                }
            }
        }
        panic!(
            "Scripted menu events must end with ENTER or ESC for `{}`",
            menu.title
        );
    }

    fn draw_frame(
        &self,
        stdout: &mut Stdout,
        menu: &MenuUI,
        selected_index: usize,
    ) -> Result<(), io::Error> {
        self.clear_screen(stdout)?;
        self.write_layout(stdout, menu, selected_index)?;
        stdout.flush()
    }

    fn write_layout(
        &self,
        writer: &mut Stdout,
        menu: &MenuUI,
        selected_index: usize,
    ) -> Result<(), io::Error> {
        for line in self.layout_lines(menu, selected_index) {
            write_line(&mut *writer, &line)?;
        }
        Ok(())
    }

    fn layout_lines(&self, menu: &MenuUI, selected_index: usize) -> Vec<String> {
        let ui = style();
        let hint = menu.footer_hint.as_deref().unwrap_or(DEFAULT_HINT);
        let label_width = menu
            .items
            .iter()
            .map(|item| display_label(&item.label).len())
            .max()
            .unwrap_or(0);
        let mut context_lines = Vec::new();
        if let Some(context) = &menu.context {
            context_lines.extend(context.lines().map(|line| line.to_string()));
        }
        let selected_enabled = menu
            .items
            .get(selected_index)
            .map(|item| item.enabled)
            .unwrap_or(false);
        let mut items = Vec::new();
        for (index, item) in menu.items.iter().enumerate() {
            let label = display_label(&item.label);
            let padded_label = format!("{:width$}", label, width = label_width);
            let highlight = selected_enabled && item.enabled && index == selected_index;
            let prefix = if highlight {
                format!("  {} ", ui.highlight_marker)
            } else {
                "    ".to_string()
            };
            let label_body = if highlight {
                padded_label.clone()
            } else {
                format_menu_label(&padded_label, item.enabled, ui.use_color)
            };
            let base = format!("{prefix}{label_body}  {}", item.description);
            let width = visible_width(&base);
            let rendered = if highlight {
                ui.apply_highlight_style(&base)
            } else {
                base
            };
            items.push((rendered, width));
        }

        let header = format_header(&menu.title);
        let mut max_width = visible_width(&header);
        for line in &context_lines {
            max_width = max_width.max(visible_width(line));
        }
        for (_, width) in &items {
            max_width = max_width.max(*width);
        }
        max_width = max_width.max(visible_width(hint));

        let rule = ui.horizontal_line(max_width);
        let mut lines = Vec::new();
        lines.push(header);
        lines.push(rule.clone());
        if !context_lines.is_empty() {
            lines.extend(context_lines.clone());
            if context_lines
                .last()
                .map(|line| !line.is_empty())
                .unwrap_or(false)
            {
                lines.push(String::new());
            }
        }
        for (rendered, _) in items {
            lines.push(rendered);
        }
        lines.push(rule);
        lines.push(hint.to_string());
        lines
    }

    fn print_snapshot(&self, menu: &MenuUI, selected_index: usize) {
        let mut stdout = io::stdout();
        for line in self.layout_lines(menu, selected_index) {
            write_line(&mut stdout, &line).expect("write snapshot layout");
        }
    }

    fn clear_screen(&self, stdout: &mut Stdout) -> Result<(), io::Error> {
        stdout.execute(terminal::Clear(ClearType::All))?;
        stdout.execute(cursor::MoveTo(0, 0))?;
        Ok(())
    }

    fn initial_index(menu: &MenuUI) -> usize {
        if menu.items.is_empty() {
            return 0;
        }
        let mut index = menu.initial_index.unwrap_or(0);
        if index >= menu.items.len() {
            index = menu.items.len() - 1;
        }
        Self::first_enabled_from(&menu.items, index).unwrap_or(index)
    }

    fn has_enabled(items: &[MenuUIItem]) -> bool {
        items.iter().any(|item| item.enabled)
    }

    fn first_enabled_from(items: &[MenuUIItem], start: usize) -> Option<usize> {
        if items.is_empty() {
            return None;
        }
        let len = items.len();
        let mut idx = start % len;
        for _ in 0..len {
            if items[idx].enabled {
                return Some(idx);
            }
            idx = (idx + 1) % len;
        }
        None
    }

    fn next_enabled_index(items: &[MenuUIItem], current: usize) -> usize {
        if items.is_empty() || !Self::has_enabled(items) {
            return current;
        }
        let len = items.len();
        let mut idx = current;
        for _ in 0..len {
            idx = (idx + 1) % len;
            if items[idx].enabled {
                return idx;
            }
        }
        current
    }

    fn previous_enabled_index(items: &[MenuUIItem], current: usize) -> usize {
        if items.is_empty() || !Self::has_enabled(items) {
            return current;
        }
        let len = items.len();
        let mut idx = current;
        for _ in 0..len {
            idx = if idx == 0 { len - 1 } else { idx - 1 };
            if items[idx].enabled {
                return idx;
            }
        }
        current
    }

    fn first_enabled_index(items: &[MenuUIItem]) -> Option<usize> {
        items.iter().position(|item| item.enabled)
    }

    fn last_enabled_index(items: &[MenuUIItem]) -> Option<usize> {
        items.iter().rposition(|item| item.enabled)
    }

    fn page_up_index(items: &[MenuUIItem], current: usize) -> usize {
        if items.is_empty() || !Self::has_enabled(items) {
            return current;
        }
        let mut idx = current;
        for _ in 0..3 {
            let next = Self::previous_enabled_index(items, idx);
            if next == idx {
                break;
            }
            idx = next;
        }
        idx
    }

    fn page_down_index(items: &[MenuUIItem], current: usize) -> usize {
        if items.is_empty() || !Self::has_enabled(items) {
            return current;
        }
        let mut idx = current;
        for _ in 0..3 {
            let next = Self::next_enabled_index(items, idx);
            if next == idx {
                break;
            }
            idx = next;
        }
        idx
    }
}

fn display_label(label: &str) -> String {
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn format_menu_label(label: &str, enabled: bool, use_color: bool) -> String {
    if !use_color {
        return label.to_string();
    }
    if enabled {
        label.white().to_string()
    } else {
        label.dark_grey().italic().to_string()
    }
}
