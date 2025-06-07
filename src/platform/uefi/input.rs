#[cfg(feature = "uefi")]
use uefi::proto::console::text::{Input, Key, ScanCode};

use crate::platform::KeyEvent;
use std::io::Result;

pub struct UefiInput;

impl UefiInput {
    #[cfg(feature = "uefi")]
    pub fn read_key(stdin: &mut Input) -> Result<KeyEvent> {
        loop {
            let key_event = stdin.wait_for_key_stroke().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
            })?;

            match key_event {
                Key::Printable(unicode_char) => {
                    let ch = unicode_char.into();
                    
                    // Check for control characters
                    match ch {
                        '\r' => return Ok(KeyEvent::Enter),
                        '\x08' => return Ok(KeyEvent::Backspace),
                        '\t' => return Ok(KeyEvent::Tab),
                        '\x1b' => return Ok(KeyEvent::Escape),
                        '\x13' => return Ok(KeyEvent::CtrlS),    // Ctrl+S
                        '\x0c' => return Ok(KeyEvent::CtrlL),    // Ctrl+L
                        '\x0e' => return Ok(KeyEvent::CtrlN),    // Ctrl+N
                        '\x09' => return Ok(KeyEvent::CtrlI),    // Ctrl+I
                        '\x02' => return Ok(KeyEvent::CtrlB),    // Ctrl+B
                        '\x0b' => return Ok(KeyEvent::CtrlK),    // Ctrl+K
                        '\x1a' => return Ok(KeyEvent::CtrlZ),    // Ctrl+Z
                        '\x19' => return Ok(KeyEvent::CtrlY),    // Ctrl+Y
                        c if c.is_control() => continue,  // Skip other control chars
                        c => return Ok(KeyEvent::Char(c)),
                    }
                }
                Key::Special(scan_code) => {
                    match scan_code {
                        ScanCode::UP => return Ok(KeyEvent::Up),
                        ScanCode::DOWN => return Ok(KeyEvent::Down),
                        ScanCode::LEFT => return Ok(KeyEvent::Left),
                        ScanCode::RIGHT => return Ok(KeyEvent::Right),
                        ScanCode::HOME => return Ok(KeyEvent::Home),
                        ScanCode::END => return Ok(KeyEvent::End),
                        ScanCode::PAGE_UP => return Ok(KeyEvent::PageUp),
                        ScanCode::PAGE_DOWN => return Ok(KeyEvent::PageDown),
                        ScanCode::DELETE => return Ok(KeyEvent::Delete),
                        ScanCode::ESCAPE => return Ok(KeyEvent::Escape),
                        _ => continue,  // Skip unknown scan codes
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "uefi"))]
    pub fn read_key(_stdin: &mut ()) -> Result<KeyEvent> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "UEFI input not available",
        ))
    }

    #[cfg(feature = "uefi")]
    pub fn flush_buffer(stdin: &mut Input) -> Result<()> {
        // Flush any pending key strokes
        while stdin.read_key_stroke().is_ok() {
            // Continue reading until buffer is empty
        }
        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn wait_for_any_key(stdin: &mut Input) -> Result<KeyEvent> {
        Self::read_key(stdin)
    }

    #[cfg(feature = "uefi")]
    pub fn is_key_available(stdin: &Input) -> bool {
        // Check if a key is available without blocking
        stdin.read_key_stroke().is_ok()
    }
}