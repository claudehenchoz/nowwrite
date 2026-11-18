#[cfg(feature = "terminal")]
pub mod terminal;
#[cfg(feature = "uefi")]
pub mod uefi;
#[cfg(test)]
pub mod mock;

use crate::error::Result;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    /// A character typed with AltGr (right Alt) held, before layout mapping.
    /// Only platforms that report US-layout characters produce this.
    AltGr(char),
    Enter,
    Backspace,
    Delete,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    CtrlS,
    CtrlL,
    CtrlN,
    CtrlI,
    CtrlB,
    CtrlK,
    CtrlZ,
    CtrlY,
    /// Theme picker (Ctrl+T, F6).
    CtrlT,
    /// Quit (Ctrl+Q, F10).
    CtrlQ,
    /// Show the help overlay (F1).
    Help,
    // Cursor movement with Shift held: extends the selection.
    ShiftUp,
    ShiftDown,
    ShiftLeft,
    ShiftRight,
    ShiftHome,
    ShiftEnd,
    /// Move by word (Ctrl+Left / Ctrl+Right).
    CtrlLeft,
    CtrlRight,
    /// Extend the selection by word (Ctrl+Shift+Left / Ctrl+Shift+Right).
    CtrlShiftLeft,
    CtrlShiftRight,
    /// The screen size changed; nothing to do except redraw.
    Resize,
    /// No key arrived within the platform's tick interval. Drives the
    /// software cursor blink and idle backups.
    Tick,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    pub level: u8,
    pub charging: bool,
}

/// Wall-clock time as reported by the platform (local time where available).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Timestamp {
    /// `YYYYMMDD_HHMMSS`, used in backup file names.
    pub fn compact(&self) -> String {
        alloc::format!(
            "{:04}{:02}{:02}_{:02}{:02}{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// Seconds since 1970-01-01, for measuring intervals.
    pub fn to_seconds(&self) -> u64 {
        // Days from civil date (Howard Hinnant's algorithm).
        let (y, m, d) = (i64::from(self.year), i64::from(self.month), i64::from(self.day));
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let secs = days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second);
        secs.max(0) as u64
    }
}

/// Everything the editor needs from the environment it runs in.
///
/// Drawing goes through the ratatui `Backend` created by `init`; the
/// platform itself handles input, files, time and power.
pub trait Platform: Sized {
    type Backend: ratatui::backend::Backend;

    /// Whether the editor should apply its own keyboard-layout remapping.
    /// False where the OS already applies the user's layout.
    const REMAP_KEYBOARD_LAYOUT: bool;

    /// Whether the editor should draw (and blink) its own cursor instead of
    /// using the backend's hardware cursor. Requires `read_key` to return
    /// `KeyEvent::Tick` about twice a second while idle.
    const SOFTWARE_CURSOR: bool;

    fn init() -> Result<(Self, Self::Backend)>;
    fn read_key(&mut self) -> Result<KeyEvent>;

    /// A key that is already waiting, without blocking (never `Tick`).
    /// Lets the editor handle a burst of keys before drawing once.
    fn poll_key(&mut self) -> Result<Option<KeyEvent>> {
        Ok(None)
    }

    /// List `.md` files and directories in `dir`, sorted by name.
    fn list_files(&mut self, dir: &str) -> Result<Vec<FileInfo>>;
    fn read_file(&mut self, path: &str) -> Result<String>;
    fn write_file(&mut self, path: &str, content: &str) -> Result<()>;
    fn create_directory(&mut self, path: &str) -> Result<()>;
    fn exists(&mut self, path: &str) -> bool;

    fn now(&self) -> Timestamp;
    fn battery_status(&self) -> Option<BatteryStatus>;
}

#[cfg(test)]
mod tests {
    use super::Timestamp;

    #[test]
    fn timestamp_to_seconds() {
        let t = Timestamp { year: 1970, month: 1, day: 1, hour: 0, minute: 0, second: 0 };
        assert_eq!(t.to_seconds(), 0);
        let t = Timestamp { year: 2024, month: 3, day: 15, hour: 14, minute: 30, second: 22 };
        assert_eq!(t.to_seconds(), 1_710_513_022);
        assert_eq!(t.compact(), "20240315_143022");
    }
}
