#[cfg(feature = "uefi")]
use uefi::proto::console::text::Output;
#[cfg(feature = "uefi")]
use uefi::proto::console::text::Color;

use crate::platform::TextAttribute;
use std::io::Result;

pub struct UefiDisplay;

impl UefiDisplay {
    #[cfg(feature = "uefi")]
    pub fn get_screen_size(stdout: &Output) -> (usize, usize) {
        let mode = stdout.current_mode().unwrap();
        let info = stdout.query_mode(mode).unwrap();
        (info.columns(), info.rows())
    }

    #[cfg(not(feature = "uefi"))]
    pub fn get_screen_size(_stdout: &()) -> (usize, usize) {
        (80, 24)
    }

    #[cfg(feature = "uefi")]
    pub fn clear_screen(stdout: &mut Output) -> Result<()> {
        stdout.clear().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;
        Ok(())
    }

    #[cfg(not(feature = "uefi"))]
    pub fn clear_screen(_stdout: &mut ()) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn draw_char(
        stdout: &mut Output,
        x: usize,
        y: usize,
        ch: char,
        attr: TextAttribute,
    ) -> Result<()> {
        stdout.set_cursor_position(x, y).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let foreground = if attr.bright {
            Color::LightGray
        } else {
            Color::White
        };

        let background = match attr.background {
            crate::platform::Color::Default => Color::Black,
            crate::platform::Color::Grey => Color::DarkGray,
        };

        stdout.set_color(foreground, background).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let mut buffer = [0u16; 2];
        let encoded = ch.encode_utf16(&mut buffer);
        
        for &code_unit in encoded {
            stdout.write_char(code_unit as char).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
            })?;
        }

        Ok(())
    }

    #[cfg(not(feature = "uefi"))]
    pub fn draw_char(
        _stdout: &mut (),
        _x: usize,
        _y: usize,
        _ch: char,
        _attr: TextAttribute,
    ) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn set_mode(stdout: &mut Output, mode: usize) -> Result<()> {
        stdout.set_mode(mode).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;
        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn reset(stdout: &mut Output) -> Result<()> {
        stdout.reset(false).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;
        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn enable_cursor(stdout: &mut Output, visible: bool) -> Result<()> {
        stdout.enable_cursor(visible).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;
        Ok(())
    }
}