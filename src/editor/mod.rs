pub mod buffer;
pub mod cursor;
pub mod markdown;

use crate::config::keyboard::KeyboardConfig;
use crate::error::{Error, Result};
use crate::file::backup::BackupManager;
use crate::file::{file_name, manager};
use crate::platform::{KeyEvent, Platform};
use crate::ui::editor_view::EditorView;
use crate::ui::modal::{self, FileAction, FileModal, KeyboardModal, NamePurpose};
use crate::ui::status_bar::{self, StatusInfo};
use alloc::boxed::Box;
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
    /// "Save changes?" before doing `Pending`.
    Confirm(Pending),
    /// Help overlay; restores the boxed mode when closed.
    Help(Box<Mode>),
}

/// An action that would replace or close the current document.
#[derive(Debug, Clone, Copy)]
enum Pending {
    Quit,
    Open,
    New,
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
    /// Action to continue with once a save (possibly via "Save As") succeeds.
    after_save: Option<Pending>,
    /// Column to aim for while moving up/down through rows of different length.
    goal_x: Option<usize>,
    /// Blink phase of the software cursor (see `Platform::SOFTWARE_CURSOR`).
    cursor_on: bool,
    /// Ignore the next blink tick, so the cursor stays visible for at least
    /// one full blink period after a key press.
    skip_tick: bool,
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
            after_save: None,
            goal_x: None,
            cursor_on: true,
            skip_tick: false,
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
        let layout = P::REMAP_KEYBOARD_LAYOUT.then(|| self.keyboard.current().code);
        let info = StatusInfo {
            message: self.message.as_deref(),
            layout,
            pending_dead: self.keyboard.pending_dead(),
            battery: self.platform.battery_status(),
        };
        let document = String::from(self.buffer.file_path().map_or("this new document", file_name));
        let cursor_on = self.cursor_on;
        let Self { terminal, buffer, view, mode, .. } = self;
        terminal
            .draw(|frame| {
                let [main, status] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
                let editor_cursor = view.render(frame, main, buffer);
                status_bar::render(frame, status, buffer, &info);
                let cursor = match mode {
                    Mode::Editing => editor_cursor,
                    Mode::Files(modal) => modal.render(frame),
                    Mode::Keyboard(modal) => {
                        modal.render(frame);
                        None
                    }
                    Mode::Confirm(_) => {
                        modal::render_confirm(frame, &document);
                        None
                    }
                    Mode::Help(_) => {
                        modal::render_help(frame, layout);
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
                if self.skip_tick {
                    self.skip_tick = false;
                } else {
                    self.cursor_on = !self.cursor_on;
                }
                return;
            }
            KeyEvent::Resize => return,
            _ => {
                // Keep the cursor visible while typing.
                self.cursor_on = true;
                self.skip_tick = true;
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
        // Any key closes the help overlay.
        if let Mode::Help(_) = self.mode {
            if let Mode::Help(previous) = core::mem::replace(&mut self.mode, Mode::Editing) {
                self.mode = *previous;
            }
            return;
        }
        if key == KeyEvent::Help {
            let previous = core::mem::replace(&mut self.mode, Mode::Editing);
            self.mode = Mode::Help(Box::new(previous));
            return;
        }

        match &mut self.mode {
            Mode::Editing => self.handle_editing_key(key),
            Mode::Files(modal) => {
                if key == KeyEvent::CtrlQ {
                    self.mode = Mode::Editing;
                    self.request(Pending::Quit);
                    return;
                }
                let Some(action) = modal.handle_key(key) else { return };
                if let FileAction::Create(name) | FileAction::SaveAs(name) = &action {
                    if self.platform.exists(name) {
                        let error = format!("{name} already exists. Choose another name.");
                        if let Mode::Files(modal) = &mut self.mode {
                            modal.set_error(error);
                        }
                        return;
                    }
                }
                self.mode = Mode::Editing;
                self.handle_file_action(action);
            }
            Mode::Keyboard(modal) => {
                if let Some(choice) = modal.handle_key(key) {
                    self.mode = Mode::Editing;
                    if let Some(code) = choice {
                        match self.keyboard.set_layout(&mut self.platform, code) {
                            Ok(()) => self.message = Some(format!("Keyboard layout: {code}")),
                            Err(err) => self.message = Some(format!("Could not save layout: {err}")),
                        }
                    }
                }
            }
            Mode::Confirm(pending) => {
                let pending = *pending;
                match key {
                    KeyEvent::Char('s' | 'S') | KeyEvent::CtrlS => {
                        self.mode = Mode::Editing;
                        self.after_save = Some(pending);
                        self.save();
                    }
                    KeyEvent::Char('d' | 'D') => {
                        self.mode = Mode::Editing;
                        self.perform(pending);
                    }
                    KeyEvent::Escape => self.mode = Mode::Editing,
                    _ => {}
                }
            }
            Mode::Help(_) => {}
        }
    }

    /// Do `pending`, asking to save first if there are unsaved changes.
    fn request(&mut self, pending: Pending) {
        if self.buffer.is_modified() {
            self.mode = Mode::Confirm(pending);
        } else {
            self.perform(pending);
        }
    }

    fn perform(&mut self, pending: Pending) {
        match pending {
            Pending::Quit => self.running = false,
            Pending::Open => self.open_file_modal(None),
            Pending::New => self.open_file_modal(Some(NamePurpose::Create)),
        }
    }

    fn handle_file_action(&mut self, action: FileAction) {
        match action {
            FileAction::Open(name) => match manager::load(&mut self.platform, &name) {
                Ok(buffer) => self.buffer = buffer,
                Err(err) => self.message = Some(format!("Could not open {name}: {err}")),
            },
            FileAction::Create(name) => {
                let mut buffer = TextBuffer::new();
                buffer.set_file_path(name.clone());
                match manager::save(&mut self.platform, &mut buffer) {
                    Ok(()) => {
                        self.buffer = buffer;
                        self.message = Some(format!("Created {name}"));
                    }
                    Err(err) => self.message = Some(format!("Could not create {name}: {err}")),
                }
            }
            FileAction::SaveAs(name) => {
                self.buffer.set_file_path(name);
                self.save();
            }
            FileAction::Cancel => self.after_save = None,
        }
    }

    fn handle_editing_key(&mut self, key: KeyEvent) {
        if let Some(target) = self.movement_target(key) {
            let selecting = matches!(
                key,
                KeyEvent::ShiftUp
                    | KeyEvent::ShiftDown
                    | KeyEvent::ShiftLeft
                    | KeyEvent::ShiftRight
                    | KeyEvent::ShiftHome
                    | KeyEvent::ShiftEnd
            );
            if selecting {
                self.buffer.move_cursor_with_selection(target);
            } else {
                self.buffer.move_cursor(target);
            }
            return;
        }
        self.goal_x = None;

        match key {
            KeyEvent::Escape => {
                self.message = Some("Ctrl+Q (or F10) quits, F1 shows help".to_string());
            }
            KeyEvent::Char(ch) => self.buffer.insert_char(ch),
            KeyEvent::Enter => self.buffer.insert_char('\n'),
            KeyEvent::Tab => self.buffer.insert_str("    "),
            KeyEvent::Backspace => self.buffer.delete_char(),
            KeyEvent::Delete => self.buffer.delete_forward(),
            KeyEvent::CtrlS => {
                self.after_save = None;
                self.save();
            }
            KeyEvent::CtrlL => self.request(Pending::Open),
            KeyEvent::CtrlN => self.request(Pending::New),
            KeyEvent::CtrlQ => self.request(Pending::Quit),
            KeyEvent::CtrlK if P::REMAP_KEYBOARD_LAYOUT => {
                self.mode = Mode::Keyboard(KeyboardModal::new(self.keyboard.current()));
            }
            KeyEvent::CtrlK => {
                self.message = Some("This version uses your system's keyboard layout".to_string());
            }
            KeyEvent::CtrlZ => {
                if !self.buffer.undo() {
                    self.message = Some("Nothing to undo".to_string());
                }
            }
            KeyEvent::CtrlY => {
                if !self.buffer.redo() {
                    self.message = Some("Nothing to redo".to_string());
                }
            }
            KeyEvent::CtrlB => self.buffer.toggle_wrap("**"),
            KeyEvent::CtrlI => self.buffer.toggle_wrap("*"),
            _ => {}
        }
    }

    /// Where a cursor-movement key moves the cursor (`None` for other keys).
    /// Up/Down/Home/End work on screen rows of soft-wrapped lines.
    fn movement_target(&mut self, key: KeyEvent) -> Option<Cursor> {
        let cursor = self.buffer.cursor();
        let rows = match key {
            KeyEvent::Up | KeyEvent::ShiftUp => Some(-1),
            KeyEvent::Down | KeyEvent::ShiftDown => Some(1),
            KeyEvent::PageUp => Some(-(self.view.page_height() as isize)),
            KeyEvent::PageDown => Some(self.view.page_height() as isize),
            _ => None,
        };
        if let Some(rows) = rows {
            let goal = *self.goal_x.get_or_insert_with(|| self.view.cursor_x(&self.buffer));
            return Some(self.view.vertical(&self.buffer, rows, goal));
        }

        self.goal_x = None;
        Some(match key {
            KeyEvent::Left | KeyEvent::ShiftLeft => {
                if cursor.column > 0 {
                    Cursor::new_at(cursor.line, cursor.column - 1)
                } else if cursor.line > 0 {
                    Cursor::new_at(cursor.line - 1, self.buffer.line_len(cursor.line - 1))
                } else {
                    cursor
                }
            }
            KeyEvent::Right | KeyEvent::ShiftRight => {
                if cursor.column < self.buffer.line_len(cursor.line) {
                    Cursor::new_at(cursor.line, cursor.column + 1)
                } else if cursor.line + 1 < self.buffer.lines().len() {
                    Cursor::new_at(cursor.line + 1, 0)
                } else {
                    cursor
                }
            }
            KeyEvent::Home | KeyEvent::ShiftHome => self.view.row_edge(&self.buffer, false),
            KeyEvent::End | KeyEvent::ShiftEnd => self.view.row_edge(&self.buffer, true),
            _ => return None,
        })
    }

    fn open_file_modal(&mut self, naming: Option<NamePurpose>) {
        match manager::list_documents(&mut self.platform) {
            Ok(files) => {
                self.mode = Mode::Files(match naming {
                    Some(purpose) => FileModal::naming(files, purpose),
                    None => FileModal::new(files),
                });
            }
            Err(err) => self.message = Some(format!("Could not list files: {err}")),
        }
    }

    /// Back up, then save. A document without a name asks for one first;
    /// `after_save` continues once the save succeeds.
    fn save(&mut self) {
        if self.buffer.file_path().is_none() {
            self.mode = Mode::Files(FileModal::naming(Vec::new(), NamePurpose::SaveAs));
            return;
        }
        let result = self
            .backups
            .backup(&mut self.platform, &self.buffer)
            .and_then(|()| manager::save(&mut self.platform, &mut self.buffer));
        match result {
            Ok(()) => {
                self.message = Some(format!("Saved {}", file_name(self.buffer.file_path().unwrap_or(""))));
                if let Some(pending) = self.after_save.take() {
                    self.perform(pending);
                }
            }
            Err(err) => {
                self.message = Some(format!("Save failed: {err}"));
                self.after_save = None;
            }
        }
    }
}
