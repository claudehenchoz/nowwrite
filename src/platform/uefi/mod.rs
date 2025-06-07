pub mod display;
pub mod input;
pub mod filesystem;
pub mod power;

use super::{Platform, KeyEvent, TextAttribute, FileInfo, PowerMode, BatteryStatus};
use std::io::Result;

#[cfg(feature = "uefi")]
use uefi::prelude::*;
#[cfg(feature = "uefi")]
use uefi::proto::console::text::{Input, Output};
#[cfg(feature = "uefi")]
use uefi::proto::media::fs::SimpleFileSystem;

#[cfg(feature = "uefi")]
pub struct UefiPlatform {
    system_table: SystemTable<Boot>,
    stdout: &'static mut Output,
    stdin: &'static mut Input,
    file_system: Option<&'static mut SimpleFileSystem>,
}

#[cfg(not(feature = "uefi"))]
pub struct UefiPlatform {
    // Stub for non-UEFI builds
}

#[cfg(feature = "uefi")]
impl Platform for UefiPlatform {
    fn init() -> Result<Self> {
        // This would be called from UEFI entry point
        // Implementation depends on UEFI boot services
        todo!("UEFI platform initialization")
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        use input::UefiInput;
        UefiInput::read_key(self.stdin)
    }

    fn get_screen_size(&self) -> (usize, usize) {
        use display::UefiDisplay;
        UefiDisplay::get_screen_size(self.stdout)
    }

    fn clear_screen(&mut self) -> Result<()> {
        use display::UefiDisplay;
        UefiDisplay::clear_screen(self.stdout)
    }

    fn draw_char(&mut self, x: usize, y: usize, ch: char, attr: TextAttribute) -> Result<()> {
        use display::UefiDisplay;
        UefiDisplay::draw_char(self.stdout, x, y, ch, attr)
    }

    fn list_files(&self, path: &str) -> Result<Vec<FileInfo>> {
        use filesystem::UefiFileSystem;
        if let Some(fs) = self.file_system {
            UefiFileSystem::list_files(fs, path)
        } else {
            Ok(Vec::new())
        }
    }

    fn read_file(&self, path: &str) -> Result<String> {
        use filesystem::UefiFileSystem;
        if let Some(fs) = self.file_system {
            UefiFileSystem::read_file(fs, path)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No file system available",
            ))
        }
    }

    fn write_file(&self, path: &str, content: &str) -> Result<()> {
        use filesystem::UefiFileSystem;
        if let Some(fs) = self.file_system {
            UefiFileSystem::write_file(fs, path, content)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No file system available",
            ))
        }
    }

    fn create_directory(&self, path: &str) -> Result<()> {
        use filesystem::UefiFileSystem;
        if let Some(fs) = self.file_system {
            UefiFileSystem::create_directory(fs, path)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No file system available",
            ))
        }
    }

    fn get_battery_status(&self) -> Option<BatteryStatus> {
        use power::UefiPower;
        UefiPower::get_battery_status(&self.system_table)
    }

    fn set_power_mode(&mut self, mode: PowerMode) -> Result<()> {
        use power::UefiPower;
        UefiPower::set_power_mode(&mut self.system_table, mode)
    }
}

#[cfg(not(feature = "uefi"))]
impl Platform for UefiPlatform {
    fn init() -> Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "UEFI platform not available in this build",
        ))
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "UEFI platform not available",
        ))
    }

    fn get_screen_size(&self) -> (usize, usize) {
        (80, 24)
    }

    fn clear_screen(&mut self) -> Result<()> {
        Ok(())
    }

    fn draw_char(&mut self, _x: usize, _y: usize, _ch: char, _attr: TextAttribute) -> Result<()> {
        Ok(())
    }

    fn list_files(&self, _path: &str) -> Result<Vec<FileInfo>> {
        Ok(Vec::new())
    }

    fn read_file(&self, _path: &str) -> Result<String> {
        Ok(String::new())
    }

    fn write_file(&self, _path: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    fn create_directory(&self, _path: &str) -> Result<()> {
        Ok(())
    }

    fn get_battery_status(&self) -> Option<BatteryStatus> {
        None
    }

    fn set_power_mode(&mut self, _mode: PowerMode) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "uefi")]
impl UefiPlatform {
    pub fn from_system_table(
        system_table: SystemTable<Boot>,
        stdout: &'static mut Output,
        stdin: &'static mut Input,
        file_system: Option<&'static mut SimpleFileSystem>,
    ) -> Self {
        Self {
            system_table,
            stdout,
            stdin,
            file_system,
        }
    }
}