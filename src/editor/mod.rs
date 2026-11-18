pub mod buffer;
pub mod cursor;
pub mod markdown;

use crate::config::keyboard::KeyboardConfig;
use crate::error::{Error, Result};
use crate::perf::{self, Phase};
use crate::file::backup::BackupManager;
use crate::file::{file_name, manager};
use crate::platform::{KeyEvent, Platform};
use crate::ui::editor_view::EditorView;
use crate::config::theme as theme_config;
use crate::ui::modal::{self, FileAction, FileModal, KeyboardModal, NamePurpose, ThemeAction, ThemeModal};
use crate::ui::status_bar::{self, StatusInfo};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use buffer::TextBuffer;
use cursor::Cursor;
use ratatui::Terminal;

enum Mode {
    Editing,
    Files(FileModal),
    Keyboard(KeyboardModal),
    Theme(ThemeModal),
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
    /// `message` is an error (shown in the theme's error colour).
    message_is_error: bool,
    /// Repaint every cell on the next draw (after a theme change).
    repaint: bool,
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
    /// Keys handled since the last backup check (see `run`).
    keys_since_backup_check: u32,
}

/// Check for a due backup at least this often while typing without pause.
const BACKUP_CHECK_KEYS: u32 = 64;
/// Keys handled between two frames at most, so the screen still updates
/// during a long burst.
const MAX_KEYS_PER_FRAME: usize = 32;

impl<P: Platform> Editor<P> {
    pub fn new(mut platform: P, backend: P::Backend) -> Result<Self> {
        // Load the theme first: clearing paints the screen in its background.
        theme_config::load(&mut platform);
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
            message_is_error: false,
            repaint: false,
            after_save: None,
            goal_x: None,
            cursor_on: true,
            skip_tick: false,
            running: true,
            keys_since_backup_check: BACKUP_CHECK_KEYS,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        while self.running {
            // Reading the clock is slow on firmware, so check for a due
            // backup while idle (each Tick) and every so many keys.
            if self.keys_since_backup_check >= BACKUP_CHECK_KEYS {
                self.keys_since_backup_check = 0;
                let start = perf::now();
                if let Err(err) = self.backups.check_and_backup(&mut self.platform, &self.buffer) {
                    self.fail(format!("Backup failed: {err}"));
                }
                perf::record(Phase::Backup, start);
            }
            let start = perf::now();
            self.draw()?;
            perf::record(Phase::Frame, start);
            let key = self.platform.read_key()?;
            self.keys_since_backup_check =
                if key == KeyEvent::Tick { BACKUP_CHECK_KEYS } else { self.keys_since_backup_check + 1 };
            let start = perf::now();
            self.handle_key(key);
            perf::record(Phase::Key, start);
            // Handle keys that arrived meanwhile (fast typing, key repeat)
            // before drawing, so the screen catches up in one frame.
            for _ in 0..MAX_KEYS_PER_FRAME {
                if !self.running {
                    break;
                }
                let Some(key) = self.platform.poll_key()? else { break };
                self.keys_since_backup_check += 1;
                let start = perf::now();
                self.handle_key(key);
                perf::record(Phase::Key, start);
            }
        }
        #[cfg(feature = "perf")]
        let _ = self.platform.write_file(".nowwrite/perf.log", &perf::report());
        self.terminal.clear().map_err(Error::io)?;
        Ok(())
    }

    /// Show `text` in the status bar until the next key.
    fn say(&mut self, text: impl Into<String>) {
        self.message = Some(text.into());
        self.message_is_error = false;
    }

    /// Show an error in the status bar until the next key.
    fn fail(&mut self, text: impl Into<String>) {
        self.message = Some(text.into());
        self.message_is_error = true;
    }

    fn draw(&mut self) -> Result<()> {
        if core::mem::take(&mut self.repaint) {
            // ratatui only redraws changed cells; a new theme changes them all.
            self.terminal.clear().map_err(Error::io)?;
        }
        let layout = P::REMAP_KEYBOARD_LAYOUT.then(|| self.keyboard.current().code);
        let info = StatusInfo {
            message: self.message.as_deref(),
            message_is_error: self.message_is_error,
            layout,
            pending_dead: self.keyboard.pending_dead(),
            battery: self.platform.battery_status(),
        };
        let document = String::from(self.buffer.file_path().map_or("this new document", file_name));
        let cursor_on = self.cursor_on;
        let Self { terminal, buffer, view, mode, .. } = self;
        terminal
            .draw(|frame| {
                let start = perf::now();
                let (main, status) = crate::ui::split_last_row(frame.area());
                let t = perf::now();
                let editor_cursor = view.render(frame, main, buffer);
                perf::record(Phase::View, t);
                let t = perf::now();
                status_bar::render(frame, status, buffer, &info);
                perf::record(Phase::StatusBar, t);
                let cursor = match mode {
                    Mode::Editing => editor_cursor,
                    Mode::Files(modal) => modal.render(frame),
                    Mode::Keyboard(modal) => {
                        modal.render(frame);
                        None
                    }
                    Mode::Theme(modal) => {
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

                perf::record(Phase::Render, start);
                let Some(position) = cursor else { return };
                if !P::SOFTWARE_CURSOR {
                    frame.set_cursor_position(position);
                } else if cursor_on {
                    if let Some(cell) = frame.buffer_mut().cell_mut(position) {
                        cell.set_style(crate::ui::cursor());
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
                            Ok(()) => self.say(format!("Keyboard layout: {code}")),
                            Err(err) => self.fail(format!("Could not save layout: {err}")),
                        }
                    }
                }
            }
            Mode::Theme(modal) => match modal.handle_key(key) {
                None => {}
                Some(ThemeAction::Preview) => self.repaint = true,
                Some(ThemeAction::Cancelled) => {
                    self.mode = Mode::Editing;
                    self.repaint = true;
                }
                Some(ThemeAction::Chosen(theme)) => {
                    self.mode = Mode::Editing;
                    match theme_config::save(&mut self.platform, theme) {
                        Ok(()) => self.say(format!("Theme: {}", theme.name)),
                        Err(err) => self.fail(format!("Could not save theme: {err}")),
                    }
                }
            },
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
                Err(err) => self.fail(format!("Could not open {name}: {err}")),
            },
            FileAction::Create(name) => {
                let mut buffer = TextBuffer::new();
                buffer.set_file_path(name.clone());
                match manager::save(&mut self.platform, &mut buffer) {
                    Ok(()) => {
                        self.buffer = buffer;
                        self.say(format!("Created {name}"));
                    }
                    Err(err) => self.fail(format!("Could not create {name}: {err}")),
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
                    | KeyEvent::CtrlShiftLeft
                    | KeyEvent::CtrlShiftRight
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
                self.say("Ctrl+Q (or F10) quits, F1 shows help");
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
            KeyEvent::CtrlK => self.say("This version uses your system's keyboard layout"),
            KeyEvent::CtrlT => self.mode = Mode::Theme(ThemeModal::new()),
            KeyEvent::CtrlZ => {
                if !self.buffer.undo() {
                    self.say("Nothing to undo");
                }
            }
            KeyEvent::CtrlY => {
                if !self.buffer.redo() {
                    self.say("Nothing to redo");
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
            KeyEvent::CtrlLeft | KeyEvent::CtrlShiftLeft => self.buffer.word_left(),
            KeyEvent::CtrlRight | KeyEvent::CtrlShiftRight => self.buffer.word_right(),
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
            Err(err) => self.fail(format!("Could not list files: {err}")),
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
                self.say(format!("Saved {}", file_name(self.buffer.file_path().unwrap_or(""))));
                if let Some(pending) = self.after_save.take() {
                    self.perform(pending);
                }
            }
            Err(err) => {
                self.fail(format!("Save failed: {err}"));
                self.after_save = None;
            }
        }
    }
}
