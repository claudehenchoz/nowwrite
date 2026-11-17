//! Windows console platform, for developing and testing on Windows.
//!
//! Uses crossterm for raw-mode input and ANSI output. Drawing calls are
//! queued into a frame buffer that is written to the console in one go
//! right before blocking for input, which avoids flicker.

use super::{BatteryStatus, Color, FileInfo, KeyEvent, Platform, PowerMode, TextAttribute};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{self, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::cell::RefCell;
use std::fs;
use std::io::{self, Result, Write};
use std::rc::Rc;

/// Console state shared by all clones of `WindowsPlatform`.
///
/// `Editor` clones the platform into each subsystem and modals drop their
/// clones when they close, so the console must only be restored when the
/// last clone goes away.
struct ConsoleSession {
    frame: RefCell<Vec<u8>>,
}

impl Drop for ConsoleSession {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = execute!(out, ResetColor, cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Clone)]
pub struct WindowsPlatform {
    session: Rc<ConsoleSession>,
}

impl WindowsPlatform {
    fn flush_frame(&self) -> Result<()> {
        let mut frame = self.session.frame.borrow_mut();
        if frame.is_empty() {
            return Ok(());
        }
        let mut out = io::stdout().lock();
        out.write_all(&frame)?;
        out.flush()?;
        frame.clear();
        Ok(())
    }
}

fn map_key(key: event::KeyEvent) -> Option<KeyEvent> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // AltGr is reported as Ctrl+Alt on Windows; treat those as plain characters.
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    let mapped = match key.code {
        KeyCode::Char(c) if ctrl && !alt => match c.to_ascii_lowercase() {
            's' => KeyEvent::CtrlS,
            'l' => KeyEvent::CtrlL,
            'n' => KeyEvent::CtrlN,
            'i' => KeyEvent::CtrlI,
            'b' => KeyEvent::CtrlB,
            'k' => KeyEvent::CtrlK,
            'z' => KeyEvent::CtrlZ,
            'y' => KeyEvent::CtrlY,
            _ => return None,
        },
        // TextBuffer indexes lines by byte, so non-ASCII input would panic.
        // Match the Linux backend and accept printable ASCII only.
        KeyCode::Char(c) if c.is_ascii_graphic() || c == ' ' => KeyEvent::Char(c),
        KeyCode::Char(_) => return None,
        KeyCode::Enter => KeyEvent::Enter,
        KeyCode::Backspace => KeyEvent::Backspace,
        KeyCode::Delete => KeyEvent::Delete,
        KeyCode::Tab => KeyEvent::Tab,
        KeyCode::Esc => KeyEvent::Escape,
        KeyCode::Up => KeyEvent::Up,
        KeyCode::Down => KeyEvent::Down,
        KeyCode::Left => KeyEvent::Left,
        KeyCode::Right => KeyEvent::Right,
        KeyCode::Home => KeyEvent::Home,
        KeyCode::End => KeyEvent::End,
        KeyCode::PageUp => KeyEvent::PageUp,
        KeyCode::PageDown => KeyEvent::PageDown,
        _ => return None,
    };
    Some(mapped)
}

impl Platform for WindowsPlatform {
    fn init() -> Result<Self> {
        terminal::enable_raw_mode()?;
        // Create the session before touching the screen so a failure below
        // still restores the console via Drop.
        let platform = WindowsPlatform {
            session: Rc::new(ConsoleSession {
                frame: RefCell::new(Vec::new()),
            }),
        };
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        Ok(platform)
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        self.flush_frame()?;
        loop {
            if let Event::Key(key) = event::read()? {
                // Windows reports both press and release events.
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if let Some(mapped) = map_key(key) {
                    return Ok(mapped);
                }
            }
        }
    }

    fn get_screen_size(&self) -> (usize, usize) {
        terminal::size()
            .map(|(w, h)| (w as usize, h as usize))
            .unwrap_or((80, 24))
    }

    fn clear_screen(&mut self) -> Result<()> {
        let mut frame = self.session.frame.borrow_mut();
        queue!(
            *frame,
            ResetColor,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
    }

    fn draw_char(&mut self, x: usize, y: usize, ch: char, attr: TextAttribute) -> Result<()> {
        let fg = if attr.bright {
            style::Color::White
        } else {
            style::Color::Grey
        };
        let bg = match attr.background {
            Color::Default => style::Color::Reset,
            Color::Grey => style::Color::DarkGrey,
        };
        let mut frame = self.session.frame.borrow_mut();
        queue!(
            *frame,
            cursor::MoveTo(x as u16, y as u16),
            SetForegroundColor(fg),
            SetBackgroundColor(bg),
            Print(ch)
        )
    }

    fn list_files(&self, path: &str) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();

            if name.ends_with(".md") || metadata.is_dir() {
                files.push(FileInfo {
                    name,
                    is_directory: metadata.is_dir(),
                    size: metadata.len(),
                });
            }
        }

        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    fn read_file(&self, path: &str) -> Result<String> {
        fs::read_to_string(path)
    }

    fn write_file(&self, path: &str, content: &str) -> Result<()> {
        fs::write(path, content)
    }

    fn create_directory(&self, path: &str) -> Result<()> {
        fs::create_dir_all(path)
    }

    fn get_battery_status(&self) -> Option<BatteryStatus> {
        None
    }

    fn set_power_mode(&mut self, _mode: PowerMode) -> Result<()> {
        Ok(())
    }
}
