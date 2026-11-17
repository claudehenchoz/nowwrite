pub mod buffer;
pub mod cursor;
pub mod markdown;

use crate::config::keyboard::KeyboardConfig;
use crate::error::{Error, Result};
use crate::file::backup::BackupManager;
use crate::file::{file_name, manager};
use crate::platform::{KeyEvent, Platform};
use crate::ui::editor_view::EditorView;
use crate::ui::modal::{FileAction, FileModal, KeyboardModal};
use crate::ui::status_bar;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use buffer::TextBuffer;
use cursor::Cursor;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::Terminal;

enum Mode {
    Editing,
    Files(FileModal),
    Keyboard(KeyboardModal),
}

pub struct Editor<P: Platform> {
    platform: P,
    terminal: Terminal<P::Backend>,
    buffer: TextBuffer,
    view: EditorView,
    mode: Mode,
    keyboard: KeyboardConfig,
    backups: BackupManager,
    /// Shown in the status bar until the next key press.
    message: Option<String>,
    /// Set after Escape with unsaved changes; a second Escape quits.
    quit_armed: bool,
    /// Blink phase of the software cursor (see `Platform::SOFTWARE_CURSOR`).
    cursor_on: bool,
    running: bool,
}

impl<P: Platform> Editor<P> {
    pub fn new(mut platform: P, backend: P::Backend) -> Result<Self> {
        let mut terminal = Terminal::new(backend).map_err(Error::io)?;
        terminal.clear().map_err(Error::io)?;
        let keyboard = KeyboardConfig::load(&mut platform);
        let files = manager::list_documents(&mut platform).unwrap_or_default();

        Ok(Self {
            platform,
            terminal,
            buffer: TextBuffer::new(),
            view: EditorView::default(),
            mode: Mode::Files(FileModal::new(files)),
            keyboard,
            backups: BackupManager::default(),
            message: None,
            quit_armed: false,
            cursor_on: true,
            running: true,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        while self.running {
            if let Err(err) = self.backups.check_and_backup(&mut self.platform, &self.buffer) {
                self.message = Some(format!("Backup failed: {err}"));
            }
            self.draw()?;
            let key = self.platform.read_key()?;
            self.handle_key(key);
        }
        self.terminal.clear().map_err(Error::io)?;
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        let battery = self.platform.battery_status();
        let cursor_on = self.cursor_on;
        let Self { terminal, buffer, view, mode, message, .. } = self;
        terminal
            .draw(|frame| {
                let [main, status] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
                let editor_cursor = view.render(frame, main, buffer);
                status_bar::render(frame, status, buffer, battery, message.as_deref());
                let cursor = match mode {
                    Mode::Editing => editor_cursor,
                    Mode::Files(modal) => modal.render(frame),
                    Mode::Keyboard(modal) => {
                        modal.render(frame);
                        None
                    }
                };

                let Some(position) = cursor else { return };
                if !P::SOFTWARE_CURSOR {
                    frame.set_cursor_position(position);
                } else if cursor_on {
                    // Inverted block; the UEFI backend shows it as black on grey.
                    if let Some(cell) = frame.buffer_mut().cell_mut(position) {
                        cell.set_style(Style::new().fg(Color::Black).bg(Color::Gray));
                    }
                }
            })
            .map_err(Error::io)?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key {
            KeyEvent::Tick => {
                self.cursor_on = !self.cursor_on;
                return;
            }
            KeyEvent::Resize => return,
            _ => {
                // Keep the cursor visible while typing.
                self.cursor_on = true;
                self.message = None;
            }
        }

        // Apply the keyboard layout. Dead keys produce nothing until the next
        // character; a non-combining pair produces two.
        let keys: Vec<KeyEvent> = match key {
            KeyEvent::Char(ch) | KeyEvent::AltGr(ch) if P::REMAP_KEYBOARD_LAYOUT => {
                let altgr = matches!(key, KeyEvent::AltGr(_));
                self.keyboard.translate(ch, altgr).into_iter().map(KeyEvent::Char).collect()
            }
            KeyEvent::AltGr(ch) => vec![KeyEvent::Char(ch)],
            other => {
                self.keyboard.cancel_dead_key();
                vec![other]
            }
        };
        for key in keys {
            self.dispatch(key);
        }
    }

    fn dispatch(&mut self, key: KeyEvent) {
        match &mut self.mode {
            Mode::Editing => self.handle_editing_key(key),
            Mode::Files(modal) => {
                if let Some(action) = modal.handle_key(key) {
                    self.mode = Mode::Editing;
                    self.handle_file_action(action);
                }
            }
            Mode::Keyboard(modal) => {
                if let Some(choice) = modal.handle_key(key) {
                    self.mode = Mode::Editing;
                    if let Some(code) = choice {
                        match self.keyboard.set_layout(&mut self.platform, code) {
                            Ok(()) => self.message = Some(format!("Layout: {code}")),
                            Err(err) => self.message = Some(format!("Could not save layout: {err}")),
                        }
                    }
                }
            }
        }
    }

    fn handle_file_action(&mut self, action: FileAction) {
        match action {
            FileAction::Open(name) => match manager::load(&mut self.platform, &name) {
                Ok(buffer) => self.buffer = buffer,
                Err(err) => self.message = Some(format!("Could not open {name}: {err}")),
            },
            FileAction::Create(name) => {
                if self.platform.exists(&name) {
                    self.message = Some(format!("{name} already exists"));
                    return;
                }
                let mut buffer = TextBuffer::new();
                buffer.set_file_path(name.clone());
                match manager::save(&mut self.platform, &mut buffer) {
                    Ok(()) => self.buffer = buffer,
                    Err(err) => self.message = Some(format!("Could not create {name}: {err}")),
                }
            }
            FileAction::Cancel => {}
        }
    }

    fn handle_editing_key(&mut self, key: KeyEvent) {
        if key != KeyEvent::Escape {
            self.quit_armed = false;
        }
        let cursor = self.buffer.cursor();
        let line_count = self.buffer.lines().len();

        match key {
            KeyEvent::Escape => {
                if !self.buffer.is_modified() || self.quit_armed {
                    self.running = false;
                } else {
                    self.quit_armed = true;
                    self.message = Some("Unsaved changes: Esc again to quit, Ctrl+S to save".to_string());
                }
            }
            KeyEvent::Char(ch) => self.buffer.insert_char(ch),
            KeyEvent::Enter => self.buffer.insert_char('\n'),
            KeyEvent::Tab => self.buffer.insert_str("    "),
            KeyEvent::Backspace => self.buffer.delete_char(),
            KeyEvent::Delete => self.buffer.delete_forward(),
            KeyEvent::Up if cursor.line > 0 => {
                self.buffer.move_cursor(Cursor::new_at(cursor.line - 1, cursor.column));
            }
            KeyEvent::Down if cursor.line + 1 < line_count => {
                self.buffer.move_cursor(Cursor::new_at(cursor.line + 1, cursor.column));
            }
            KeyEvent::Left => {
                if cursor.column > 0 {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column - 1));
                } else if cursor.line > 0 {
                    let len = self.buffer.line_len(cursor.line - 1);
                    self.buffer.move_cursor(Cursor::new_at(cursor.line - 1, len));
                }
            }
            KeyEvent::Right => {
                if cursor.column < self.buffer.line_len(cursor.line) {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column + 1));
                } else if cursor.line + 1 < line_count {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line + 1, 0));
                }
            }
            KeyEvent::Home => self.buffer.move_cursor(Cursor::new_at(cursor.line, 0)),
            KeyEvent::End => {
                let len = self.buffer.line_len(cursor.line);
                self.buffer.move_cursor(Cursor::new_at(cursor.line, len));
            }
            KeyEvent::PageUp => {
                let line = cursor.line.saturating_sub(self.view.page_height());
                self.buffer.move_cursor(Cursor::new_at(line, cursor.column));
            }
            KeyEvent::PageDown => {
                let line = cursor.line + self.view.page_height();
                self.buffer.move_cursor(Cursor::new_at(line, cursor.column));
            }
            KeyEvent::CtrlS => self.save(),
            KeyEvent::CtrlL => self.open_file_modal(false),
            KeyEvent::CtrlN => self.open_file_modal(true),
            KeyEvent::CtrlK => self.mode = Mode::Keyboard(KeyboardModal::new(self.keyboard.current())),
            KeyEvent::CtrlZ => {
                self.buffer.undo();
            }
            KeyEvent::CtrlY => {
                self.buffer.redo();
            }
            KeyEvent::CtrlB => self.buffer.wrap_with("**"),
            KeyEvent::CtrlI => self.buffer.wrap_with("*"),
            _ => {}
        }
    }

    fn open_file_modal(&mut self, new_file: bool) {
        match manager::list_documents(&mut self.platform) {
            Ok(files) => {
                self.mode = Mode::Files(if new_file {
                    FileModal::new_file(files)
                } else {
                    FileModal::new(files)
                });
            }
            Err(err) => self.message = Some(format!("Could not list files: {err}")),
        }
    }

    /// Back up, then save. Untitled documents get a fresh `untitled*.md` name.
    fn save(&mut self) {
        if self.buffer.file_path().is_none() {
            let name = manager::unused_name(&mut self.platform, "untitled");
            self.buffer.set_file_path(name);
        }
        let result = self
            .backups
            .backup(&mut self.platform, &self.buffer)
            .and_then(|()| manager::save(&mut self.platform, &mut self.buffer));
        self.message = Some(match result {
            Ok(()) => format!("Saved {}", file_name(self.buffer.file_path().unwrap_or(""))),
            Err(err) => format!("Save failed: {err}"),
        });
    }
}
