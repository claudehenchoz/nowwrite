//! Terminal platform for Linux, Windows and macOS (feature `terminal`),
//! using ratatui's crossterm backend. Used for development and testing.

use super::{BatteryStatus, FileInfo, KeyEvent, Platform, Timestamp};
use crate::error::Result;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use chrono::{Datelike, Timelike};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{cursor, execute};
use std::fs;
use std::io::{self, Stdout};
use std::path::Path;

pub struct TerminalPlatform {
    _private: (),
}

fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
}

impl Drop for TerminalPlatform {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn map_key(key: event::KeyEvent) -> Option<KeyEvent> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // AltGr arrives as Ctrl+Alt on Windows; treat those as plain characters.
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    Some(match key.code {
        KeyCode::Char(c) if ctrl && !alt => match c.to_ascii_lowercase() {
            's' => KeyEvent::CtrlS,
            'l' => KeyEvent::CtrlL,
            'n' => KeyEvent::CtrlN,
            'i' => KeyEvent::CtrlI,
            'b' => KeyEvent::CtrlB,
            'k' => KeyEvent::CtrlK,
            'z' => KeyEvent::CtrlZ,
            'y' => KeyEvent::CtrlY,
            'q' => KeyEvent::CtrlQ,
            _ => return None,
        },
        KeyCode::Char(c) if !c.is_control() => KeyEvent::Char(c),
        KeyCode::Enter => KeyEvent::Enter,
        KeyCode::Backspace => KeyEvent::Backspace,
        KeyCode::Delete => KeyEvent::Delete,
        KeyCode::Tab => KeyEvent::Tab,
        KeyCode::Esc => KeyEvent::Escape,
        KeyCode::Left if ctrl && shift => KeyEvent::CtrlShiftLeft,
        KeyCode::Right if ctrl && shift => KeyEvent::CtrlShiftRight,
        KeyCode::Left if ctrl => KeyEvent::CtrlLeft,
        KeyCode::Right if ctrl => KeyEvent::CtrlRight,
        KeyCode::Up if shift => KeyEvent::ShiftUp,
        KeyCode::Down if shift => KeyEvent::ShiftDown,
        KeyCode::Left if shift => KeyEvent::ShiftLeft,
        KeyCode::Right if shift => KeyEvent::ShiftRight,
        KeyCode::Home if shift => KeyEvent::ShiftHome,
        KeyCode::End if shift => KeyEvent::ShiftEnd,
        KeyCode::Up => KeyEvent::Up,
        KeyCode::Down => KeyEvent::Down,
        KeyCode::Left => KeyEvent::Left,
        KeyCode::Right => KeyEvent::Right,
        KeyCode::Home => KeyEvent::Home,
        KeyCode::End => KeyEvent::End,
        KeyCode::PageUp => KeyEvent::PageUp,
        KeyCode::PageDown => KeyEvent::PageDown,
        // Same F-key aliases as on UEFI.
        KeyCode::F(1) => KeyEvent::Help,
        KeyCode::F(2) => KeyEvent::CtrlS,
        KeyCode::F(3) => KeyEvent::CtrlL,
        KeyCode::F(4) => KeyEvent::CtrlN,
        KeyCode::F(5) => KeyEvent::CtrlK,
        KeyCode::F(10) => KeyEvent::CtrlQ,
        _ => return None,
    })
}

impl Platform for TerminalPlatform {
    type Backend = CrosstermBackend<Stdout>;
    const REMAP_KEYBOARD_LAYOUT: bool = false;
    // Terminals blink their own cursor.
    const SOFTWARE_CURSOR: bool = false;

    fn init() -> Result<(Self, Self::Backend)> {
        terminal::enable_raw_mode()?;
        // From here on Drop restores the terminal, even if setup fails.
        let platform = TerminalPlatform { _private: () };
        execute!(io::stdout(), EnterAlternateScreen)?;

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(std::boxed::Box::new(move |info| {
            restore_terminal();
            previous_hook(info);
        }));

        Ok((platform, CrosstermBackend::new(io::stdout())))
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        loop {
            // Wake up periodically so idle backups still happen.
            if !event::poll(std::time::Duration::from_secs(1))? {
                return Ok(KeyEvent::Tick);
            }
            match event::read()? {
                // Windows reports releases too.
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if let Some(mapped) = map_key(key) {
                        return Ok(mapped);
                    }
                }
                Event::Resize(..) => return Ok(KeyEvent::Resize),
                _ => {}
            }
        }
    }

    fn list_files(&mut self, dir: &str) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if metadata.is_dir() || name.ends_with(".md") {
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

    fn read_file(&mut self, path: &str) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }

    fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        Ok(fs::write(path, content)?)
    }

    fn create_directory(&mut self, path: &str) -> Result<()> {
        Ok(fs::create_dir_all(path)?)
    }

    fn exists(&mut self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn now(&self) -> Timestamp {
        let now = chrono::Local::now();
        Timestamp {
            year: now.year() as u16,
            month: now.month() as u8,
            day: now.day() as u8,
            hour: now.hour() as u8,
            minute: now.minute() as u8,
            second: now.second() as u8,
        }
    }

    fn battery_status(&self) -> Option<BatteryStatus> {
        None
    }
}
