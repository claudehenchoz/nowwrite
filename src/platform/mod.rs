#[cfg(feature = "linux")]
pub mod linux;
#[cfg(feature = "uefi")]
pub mod uefi;

use std::io::Result;

#[derive(Debug, Clone, Copy)]
pub enum KeyEvent {
    Char(char),
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
}

#[derive(Debug, Clone, Copy)]
pub struct TextAttribute {
    pub bright: bool,
    pub background: Color,
}

#[derive(Debug, Clone, Copy)]
pub enum Color {
    Default,
    Grey,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum PowerMode {
    Normal,
    PowerSaving,
    Sleep,
}

#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    pub level: u8,
    pub charging: bool,
}

pub trait Platform {
    fn init() -> Result<Self> where Self: Sized;
    fn read_key(&mut self) -> Result<KeyEvent>;
    fn get_screen_size(&self) -> (usize, usize);
    fn clear_screen(&mut self) -> Result<()>;
    fn draw_char(&mut self, x: usize, y: usize, ch: char, attr: TextAttribute) -> Result<()>;
    fn list_files(&self, path: &str) -> Result<Vec<FileInfo>>;
    fn read_file(&self, path: &str) -> Result<String>;
    fn write_file(&self, path: &str, content: &str) -> Result<()>;
    fn create_directory(&self, path: &str) -> Result<()>;
    fn get_battery_status(&self) -> Option<BatteryStatus>;
    fn set_power_mode(&mut self, mode: PowerMode) -> Result<()>;
}

impl Default for TextAttribute {
    fn default() -> Self {
        Self {
            bright: false,
            background: Color::Default,
        }
    }
}