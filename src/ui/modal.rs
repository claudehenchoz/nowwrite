use crate::config::keyboard::{KeyboardLayout, LAYOUTS};
use crate::platform::{FileInfo, KeyEvent};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub enum FileAction {
    Open(String),
    /// Create a new document with this name.
    Create(String),
    Cancel,
}

/// Lists the `.md` files and lets the user open one or name a new one.
pub struct FileModal {
    files: Vec<FileInfo>,
    selected: usize,
    /// `Some` while the user is typing a name for a new file.
    new_name: Option<String>,
}

impl FileModal {
    pub fn new(files: Vec<FileInfo>) -> Self {
        Self { files, selected: 0, new_name: None }
    }

    /// Open directly in "new file" mode.
    pub fn new_file(files: Vec<FileInfo>) -> Self {
        Self { new_name: Some(String::new()), ..Self::new(files) }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<FileAction> {
        if let Some(name) = &mut self.new_name {
            match key {
                KeyEvent::Char(c) if !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') => {
                    name.push(c)
                }
                KeyEvent::Backspace => {
                    name.pop();
                }
                KeyEvent::Enter if !name.trim().is_empty() => {
                    let mut name = String::from(name.trim());
                    if !name.ends_with(".md") {
                        name.push_str(".md");
                    }
                    return Some(FileAction::Create(name));
                }
                KeyEvent::Escape => self.new_name = None,
                _ => {}
            }
            return None;
        }

        match key {
            KeyEvent::Up => self.selected = self.selected.saturating_sub(1),
            KeyEvent::Down if self.selected + 1 < self.files.len() => self.selected += 1,
            KeyEvent::Enter => {
                if let Some(file) = self.files.get(self.selected) {
                    return Some(FileAction::Open(file.name.clone()));
                }
            }
            KeyEvent::CtrlN => self.new_name = Some(String::new()),
            KeyEvent::Escape => return Some(FileAction::Cancel),
            _ => {}
        }
        None
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = super::centered(frame.area(), 60, 16);
        frame.render_widget(Clear, area);

        if let Some(name) = &self.new_name {
            let block = Block::bordered()
                .title(Line::from(" New File ").centered())
                .title_bottom(Line::from(" Enter create  Esc back ").centered());
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let prompt = format!(" Name: {name}");
            let cursor_x = inner.x + prompt.chars().count() as u16;
            frame.render_widget(Paragraph::new(prompt), inner);
            frame.set_cursor_position(Position::new(cursor_x.min(inner.right() - 1), inner.y));
            return;
        }

        let block = Block::bordered()
            .title(Line::from(" Select File ").centered().style(super::bright()))
            .title_bottom(Line::from(" Enter open  Ctrl+N new  Esc cancel ").centered());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.files.is_empty() {
            frame.render_widget(
                Paragraph::new(" No .md files here. Press Ctrl+N to create one."),
                inner,
            );
            return;
        }
        let items: Vec<ListItem> = self.files.iter().map(|f| ListItem::new(f.name.as_str())).collect();
        render_list(frame, inner, items, self.selected);
    }
}

/// Lets the user pick a keyboard layout.
pub struct KeyboardModal {
    selected: usize,
}

impl KeyboardModal {
    pub fn new(current: KeyboardLayout) -> Self {
        let selected = LAYOUTS.iter().position(|l| *l == current).unwrap_or(0);
        Self { selected }
    }

    /// Returns `Some(Some(code))` when chosen, `Some(None)` when cancelled.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Option<&'static str>> {
        match key {
            KeyEvent::Up => self.selected = self.selected.saturating_sub(1),
            KeyEvent::Down if self.selected + 1 < LAYOUTS.len() => self.selected += 1,
            KeyEvent::Enter => return Some(Some(LAYOUTS[self.selected].code)),
            KeyEvent::Escape => return Some(None),
            _ => {}
        }
        None
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = super::centered(frame.area(), 50, LAYOUTS.len() as u16 + 4);
        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .title(Line::from(" Keyboard Layout ").centered().style(super::bright()))
            .title_bottom(Line::from(" Enter select  Esc cancel ").centered());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<ListItem> = LAYOUTS
            .iter()
            .map(|l| ListItem::new(format!("{} ({})", l.name, l.code)))
            .collect();
        render_list(frame, inner, items, self.selected);
    }
}

fn render_list(frame: &mut Frame, area: Rect, items: Vec<ListItem>, selected: usize) {
    let list = List::new(items)
        .highlight_style(super::selected())
        .highlight_symbol("> ");
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}
