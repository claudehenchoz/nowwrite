//! In-memory platform for unit tests.

use super::{BatteryStatus, FileInfo, KeyEvent, Platform, Timestamp};
use crate::error::{Error, Result};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use ratatui::backend::TestBackend;

#[derive(Default)]
pub struct MockPlatform {
    pub files: BTreeMap<String, String>,
    pub dirs: BTreeSet<String>,
}

impl Platform for MockPlatform {
    type Backend = TestBackend;
    const REMAP_KEYBOARD_LAYOUT: bool = false;
    const SOFTWARE_CURSOR: bool = false;

    fn init() -> Result<(Self, Self::Backend)> {
        Ok((Self::default(), TestBackend::new(80, 25)))
    }

    fn read_key(&mut self) -> Result<KeyEvent> {
        Ok(KeyEvent::Tick)
    }

    fn list_files(&mut self, _dir: &str) -> Result<Vec<FileInfo>> {
        Ok(self
            .files
            .iter()
            .map(|(name, text)| FileInfo { name: name.clone(), is_directory: false, size: text.len() as u64 })
            .collect())
    }

    fn read_file(&mut self, path: &str) -> Result<String> {
        self.files.get(path).cloned().ok_or(Error::NotFound)
    }

    fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        self.files.insert(path.to_string(), content.to_string());
        Ok(())
    }

    fn create_directory(&mut self, path: &str) -> Result<()> {
        self.dirs.insert(path.to_string());
        Ok(())
    }

    fn exists(&mut self, path: &str) -> bool {
        self.files.contains_key(path) || self.dirs.contains(path)
    }

    fn now(&self) -> Timestamp {
        Timestamp { year: 2024, month: 1, day: 1, hour: 0, minute: 0, second: 0 }
    }

    fn battery_status(&self) -> Option<BatteryStatus> {
        None
    }
}
