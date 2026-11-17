use super::{file_name, NOWWRITE_DIR};
use crate::editor::buffer::TextBuffer;
use crate::error::Result;
use crate::platform::Platform;
use alloc::format;

/// Writes timestamped copies of the open document to `.nowwrite/`.
pub struct BackupManager {
    last_backup: Option<u64>,
    interval_seconds: u64,
}

impl Default for BackupManager {
    fn default() -> Self {
        Self {
            last_backup: None,
            interval_seconds: 300, // 5 minutes
        }
    }
}

impl BackupManager {
    /// Back up if the document has unsaved changes and the interval has
    /// passed since the last backup. Returns whether a backup was written.
    pub fn check_and_backup<P: Platform>(
        &mut self,
        platform: &mut P,
        buffer: &TextBuffer,
    ) -> Result<bool> {
        if !buffer.is_modified() || buffer.file_path().is_none() {
            return Ok(false);
        }
        let now = platform.now().to_seconds();
        if let Some(last) = self.last_backup {
            if now.saturating_sub(last) < self.interval_seconds {
                return Ok(false);
            }
        }
        self.backup(platform, buffer)?;
        Ok(true)
    }

    /// Write a backup now (also done before every manual save).
    pub fn backup<P: Platform>(&mut self, platform: &mut P, buffer: &TextBuffer) -> Result<()> {
        let Some(path) = buffer.file_path() else {
            return Ok(());
        };
        if !platform.exists(NOWWRITE_DIR) {
            platform.create_directory(NOWWRITE_DIR)?;
        }
        let now = platform.now();
        let backup_path = format!("{NOWWRITE_DIR}/{}.{}.bak", file_name(path), now.compact());
        platform.write_file(&backup_path, &buffer.to_text())?;
        self.last_backup = Some(now.to_seconds());
        Ok(())
    }
}
