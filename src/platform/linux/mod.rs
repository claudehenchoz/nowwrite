pub mod terminal;
pub mod filesystem;

use super::{Platform, KeyEvent, TextAttribute, FileInfo, PowerMode, BatteryStatus};
use std::io::{self, Result, Write};
use std::fs;

#[derive(Clone)]
pub struct LinuxPlatform {
    original_termios: Option<termios::Termios>,
}

impl Platform for LinuxPlatform {
    fn init() -> Result<Self> {
        let original_termios = if let Ok(termios) = termios::Termios::from_fd(0) {
            Some(termios)
        } else {
            None
        };
        
        if let Some(ref termios_ref) = original_termios {
            let mut raw = termios_ref.clone();
            termios::cfmakeraw(&mut raw);
            termios::tcsetattr(0, termios::TCSANOW, &raw).ok();
        }
        
        print!("\x1B[?1049h");
        io::stdout().flush()?;
        
        Ok(LinuxPlatform {
            original_termios,
        })
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        use std::io::Read;
        
        let mut buffer = [0; 1];
        io::stdin().read_exact(&mut buffer)?;
        
        let key = match buffer[0] {
            b'\x1b' => {
                let mut seq = [0; 2];
                if io::stdin().read(&mut seq).is_ok() {
                    match &seq {
                        b"[A" => KeyEvent::Up,
                        b"[B" => KeyEvent::Down,
                        b"[C" => KeyEvent::Right,
                        b"[D" => KeyEvent::Left,
                        _ => KeyEvent::Escape,
                    }
                } else {
                    KeyEvent::Escape
                }
            }
            b'\r' | b'\n' => KeyEvent::Enter,
            b'\x08' | b'\x7f' => KeyEvent::Backspace,
            b'\t' => KeyEvent::Tab,
            b'\x13' => KeyEvent::CtrlS,
            b'\x0c' => KeyEvent::CtrlL,
            b'\x0e' => KeyEvent::CtrlN,
            // b'\x09' => KeyEvent::CtrlI,  // conflicts with Tab
            b'\x02' => KeyEvent::CtrlB,
            b'\x0b' => KeyEvent::CtrlK,
            b'\x1a' => KeyEvent::CtrlZ,
            b'\x19' => KeyEvent::CtrlY,
            c if c.is_ascii_graphic() || c == b' ' => KeyEvent::Char(c as char),
            _ => return self.read_key(),
        };
        
        Ok(key)
    }

    fn get_screen_size(&self) -> (usize, usize) {
        // Use a different approach for getting terminal size
        if let Some((width, height)) = terminal_size::terminal_size() {
            (width.0 as usize, height.0 as usize)
        } else {
            (80, 24)
        }
    }

    fn clear_screen(&mut self) -> Result<()> {
        print!("\x1B[2J\x1B[H");
        io::stdout().flush()
    }

    fn draw_char(&mut self, x: usize, y: usize, ch: char, _attr: TextAttribute) -> Result<()> {
        print!("\x1B[{};{}H{}", y + 1, x + 1, ch);
        Ok(())
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

impl Drop for LinuxPlatform {
    fn drop(&mut self) {
        print!("\x1B[?1049l");
        if let Some(ref termios) = self.original_termios {
            termios::tcsetattr(0, termios::TCSANOW, termios).ok();
        }
        io::stdout().flush().ok();
    }
}