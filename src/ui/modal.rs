use crate::config::keyboard::{KeyboardLayout, LAYOUTS};
use crate::platform::{FileInfo, KeyEvent};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

const INVALID_NAME_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

pub enum FileAction {
    Open(String),
    /// Create a new, empty document with this name.
    Create(String),
    /// Save the current document under this name.
    SaveAs(String),
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamePurpose {
    Create,
    SaveAs,
}

struct Naming {
    name: String,
    purpose: NamePurpose,
    /// Opened directly for naming (Ctrl+N, first save): Esc closes the box
    /// instead of going back to the file list.
    standalone: bool,
}

/// Lists the `.md` files and lets the user open one or name a new one.
pub struct FileModal {
    files: Vec<FileInfo>,
    selected: usize,
    naming: Option<Naming>,
    error: Option<String>,
}

impl FileModal {
    pub fn new(files: Vec<FileInfo>) -> Self {
        Self { files, selected: 0, naming: None, error: None }
    }

    /// Open directly in the name prompt.
    pub fn naming(files: Vec<FileInfo>, purpose: NamePurpose) -> Self {
        let naming = Naming { name: String::new(), purpose, standalone: true };
        Self { naming: Some(naming), ..Self::new(files) }
    }

    /// Show an error in the name prompt (e.g. the name is taken).
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<FileAction> {
        if let Some(naming) = &mut self.naming {
            match key {
                KeyEvent::Char(c) if INVALID_NAME_CHARS.contains(&c) => {
                    self.error = Some(String::from("Not allowed in file names: / \\ : * ? \" < > |"));
                }
                KeyEvent::Char(c) => {
                    naming.name.push(c);
                    self.error = None;
                }
                KeyEvent::Backspace => {
                    naming.name.pop();
                    self.error = None;
                }
                KeyEvent::Enter if !naming.name.trim().is_empty() => {
                    let mut name = String::from(naming.name.trim());
                    if !name.ends_with(".md") {
                        name.push_str(".md");
                    }
                    return Some(match naming.purpose {
                        NamePurpose::Create => FileAction::Create(name),
                        NamePurpose::SaveAs => FileAction::SaveAs(name),
                    });
                }
                KeyEvent::Escape if naming.standalone => return Some(FileAction::Cancel),
                KeyEvent::Escape => {
                    self.naming = None;
                    self.error = None;
                }
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
            KeyEvent::CtrlN => {
                self.naming = Some(Naming { name: String::new(), purpose: NamePurpose::Create, standalone: false });
            }
            KeyEvent::Escape => return Some(FileAction::Cancel),
            _ => {}
        }
        None
    }

    /// Draw the modal; returns the text cursor position while naming a file.
    pub fn render(&self, frame: &mut Frame) -> Option<Position> {
        if let Some(naming) = &self.naming {
            return Some(self.render_naming(frame, naming));
        }

        let area = super::centered(frame.area(), 60, 16);
        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .title(Line::from(" NowWrite - Select File ").centered().style(super::bright()))
            .title_bottom(Line::from(" ↑↓ Select  Enter Open  Ctrl+N/F4 New  F1 Help ").centered());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.files.is_empty() {
            let text = vec![
                Line::from(" No documents on this drive yet."),
                Line::from(""),
                Line::from(" Press Ctrl+N (or F4) to create one,"),
                Line::from(" or Esc to start writing right away."),
            ];
            frame.render_widget(Paragraph::new(text), inner);
            return None;
        }
        let items: Vec<ListItem> = self.files.iter().map(|f| ListItem::new(f.name.as_str())).collect();
        render_list(frame, inner, items, self.selected);
        None
    }

    fn render_naming(&self, frame: &mut Frame, naming: &Naming) -> Position {
        let area = super::centered(frame.area(), 60, 7);
        frame.render_widget(Clear, area);
        let (title, hint) = match naming.purpose {
            NamePurpose::Create => (" New Document ", " Enter Create  Esc Cancel "),
            NamePurpose::SaveAs => (" Save As ", " Enter Save  Esc Cancel "),
        };
        let block = Block::bordered()
            .title(Line::from(title).centered().style(super::bright()))
            .title_bottom(Line::from(hint).centered());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let prompt = " File name: ";
        let mut name_line = vec![Span::raw(prompt), Span::raw(naming.name.as_str())];
        if !naming.name.ends_with(".md") {
            name_line.push(Span::styled(".md", super::muted()));
        }
        let mut text = vec![Line::from(name_line)];
        if let Some(error) = &self.error {
            text.push(Line::from(""));
            text.push(Line::from(Span::styled(format!(" {error}"), super::bright())));
        }
        frame.render_widget(Paragraph::new(text), inner);

        let x = inner.x + (prompt.chars().count() + naming.name.chars().count()) as u16;
        Position::new(x.min(inner.right().saturating_sub(1)), inner.y)
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
            .title_bottom(Line::from(" ↑↓ Select  Enter Choose  Esc Cancel ").centered());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<ListItem> = LAYOUTS
            .iter()
            .map(|l| ListItem::new(format!("{} ({})", l.name, l.code)))
            .collect();
        render_list(frame, inner, items, self.selected);
    }
}

/// "Save changes?" prompt shown before the document would be replaced or
/// the editor closed.
pub fn render_confirm(frame: &mut Frame, document: &str) {
    let area = super::centered(frame.area(), 54, 6);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(Line::from(" Unsaved Changes ").centered().style(super::bright()))
        .title_bottom(Line::from(" S Save  D Discard  Esc Cancel ").centered());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = vec![
        Line::from(format!(" Save changes to {document}?")),
        Line::from(""),
        Line::from(Span::styled(" Discarding loses everything since the last save.", super::muted())),
    ];
    frame.render_widget(Paragraph::new(text), inner);
}

/// Help overlay (F1): every shortcut, plus where files are stored.
/// `layout` is the active layout code where NowWrite applies layouts itself.
pub fn render_help(frame: &mut Frame, layout: Option<&str>) {
    let mut text = vec![
        Line::from(Span::styled(" NowWrite - distraction-free Markdown editor", super::bright())),
        Line::from(""),
        Line::from(" Ctrl+S  F2   Save                 Ctrl+Z  Undo"),
        Line::from(" Ctrl+L  F3   Open document        Ctrl+Y  Redo"),
        Line::from(" Ctrl+N  F4   New document         Ctrl+B  Bold on/off"),
        Line::from(" Ctrl+K  F5   Keyboard layout      Ctrl+I  Italic on/off"),
        Line::from(" Ctrl+Q  F10  Quit                 Shift+Arrows  Select"),
        Line::from(" F1           This help            Esc     Close a box"),
        Line::from(""),
        Line::from(" Use the F-keys if your computer ignores Ctrl shortcuts."),
        Line::from(""),
        Line::from(" Documents: .md files in the top folder of the drive."),
        Line::from(" Backups:   .nowwrite folder, every 5 min and on each save."),
    ];
    match layout {
        Some("de-CH") => {
            text.push(Line::from(""));
            text.push(Line::from(" Swiss German: AltGr+,  <    AltGr+.  >    AltGr+-  \\"));
            text.push(Line::from(" (the key left of Y isn't available before the OS starts)"));
        }
        Some(_) => {}
        None => {
            text.push(Line::from(""));
            text.push(Line::from(" Keyboard layout: this version uses your system's layout."));
        }
    }

    let height = text.len() as u16 + 2;
    let area = super::centered(frame.area(), 66, height);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(Line::from(" Help ").centered().style(super::bright()))
        .title_bottom(Line::from(" Press any key to close ").centered());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(text), inner);
}

fn render_list(frame: &mut Frame, area: Rect, items: Vec<ListItem>, selected: usize) {
    let list = List::new(items)
        .highlight_style(super::selected())
        .highlight_symbol("> ");
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}
