//! UEFI platform: graphics or text console, keyboard, and the FAT volume
//! NowWrite was booted from.

mod backend;
mod gop;
mod input;
mod text;

pub use backend::UefiBackend;

use super::{BatteryStatus, FileInfo, KeyEvent, Platform, Timestamp};
use crate::error::{Error, Result};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use input::UefiInput;
use uefi::fs::{FileSystem, PathBuf};
use uefi::proto::console::text::Color;
use uefi::{boot, runtime, system, CString16};

/// Contains `text` to force the text console (for firmware whose GOP is broken).
const DISPLAY_CONFIG: &str = ".nowwrite/.display.config";

pub struct UefiPlatform {
    fs: FileSystem,
    input: UefiInput,
}

/// Convert an editor path (`./a/b.md`, `.nowwrite/x`) to an absolute UEFI
/// path on the boot volume (`\a\b.md`).
fn uefi_path(path: &str) -> Result<PathBuf> {
    let trimmed = path.trim_start_matches("./").trim_start_matches(['/', '\\']);
    let trimmed = if trimmed == "." { "" } else { trimmed };
    let mut converted = String::from("\\");
    converted.extend(trimmed.chars().map(|c| if c == '/' { '\\' } else { c }));
    let cstr = CString16::try_from(converted.as_str())
        .map_err(|_| Error::InvalidData(format!("unsupported characters in path {path}")))?;
    Ok(PathBuf::from(cstr))
}

fn fs_err(err: uefi::fs::Error) -> Error {
    Error::Io(err.to_string())
}

impl Platform for UefiPlatform {
    type Backend = UefiBackend;
    // Firmware delivers US-layout characters, so apply the user's layout.
    const REMAP_KEYBOARD_LAYOUT: bool = true;
    // The firmware cursor doesn't blink and is erased by redraws.
    const SOFTWARE_CURSOR: bool = true;

    fn init() -> Result<(Self, Self::Backend)> {
        // The file system comes first: it holds the display setting.
        let fs = boot::get_image_file_system(boot::image_handle())
            .map_err(|e| Error::Io(format!("boot volume not accessible: {e:?}")))?;
        let mut platform = Self { fs: FileSystem::new(fs), input: UefiInput::new()? };
        let force_text = platform.read_file(DISPLAY_CONFIG).is_ok_and(|c| c.trim() == "text");
        let backend = UefiBackend::new(force_text)?;
        Ok((platform, backend))
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        self.input.read_key()
    }

    fn list_files(&mut self, dir: &str) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        for entry in self.fs.read_dir(uefi_path(dir)?).map_err(fs_err)? {
            let Ok(info) = entry else { continue };
            let name = info.file_name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            if info.is_directory() || name.ends_with(".md") {
                files.push(FileInfo { name, is_directory: info.is_directory(), size: info.file_size() });
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    fn read_file(&mut self, path: &str) -> Result<String> {
        let path = uefi_path(path)?;
        if !self.fs.try_exists(&path).unwrap_or(false) {
            return Err(Error::NotFound);
        }
        let bytes = self.fs.read(&path).map_err(fs_err)?;
        String::from_utf8(bytes).map_err(|_| Error::InvalidData(String::from("file is not UTF-8")))
    }

    fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        self.fs.write(uefi_path(path)?, content.as_bytes()).map_err(fs_err)
    }

    fn create_directory(&mut self, path: &str) -> Result<()> {
        // Create each level ourselves: uefi::fs::create_dir_all walks up to an
        // empty parent for root-level paths like `\.nowwrite` and fails.
        let mut partial = String::new();
        for component in path.split(['/', '\\']).filter(|c| !c.is_empty() && *c != ".") {
            if !partial.is_empty() {
                partial.push('/');
            }
            partial.push_str(component);
            let dir = uefi_path(&partial)?;
            if !self.fs.try_exists(&dir).unwrap_or(false) {
                self.fs.create_dir(dir).map_err(fs_err)?;
            }
        }
        Ok(())
    }

    fn exists(&mut self, path: &str) -> bool {
        uefi_path(path).is_ok_and(|p| self.fs.try_exists(p).unwrap_or(false))
    }

    fn now(&self) -> Timestamp {
        match runtime::get_time() {
            Ok(t) => Timestamp {
                year: t.year(),
                month: t.month(),
                day: t.day(),
                hour: t.hour(),
                minute: t.minute(),
                second: t.second(),
            },
            Err(_) => Timestamp { year: 1970, month: 1, day: 1, hour: 0, minute: 0, second: 0 },
        }
    }

    fn battery_status(&self) -> Option<BatteryStatus> {
        // Reading the battery needs ACPI/embedded-controller access; not implemented.
        None
    }
}

impl Drop for UefiPlatform {
    fn drop(&mut self) {
        // Leave the console in its default state for the firmware. Setting
        // the text mode again also makes the firmware's console take the
        // screen back from graphics mode (and its resolution, if changed).
        let _ = system::with_stdout(|out| -> uefi::Result {
            if let Ok(Some(mode)) = out.current_mode() {
                let _ = out.set_mode(mode);
            }
            out.set_color(Color::LightGray, Color::Black)?;
            out.clear()?;
            // Some consoles don't support toggling the cursor; that's fine.
            let _ = out.enable_cursor(true);
            Ok(())
        });
    }
}
