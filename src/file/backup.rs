use crate::platform::Platform;
use crate::editor::buffer::TextBuffer;
use std::io::Result;
use std::path::Path;
use std::time::SystemTime;

pub struct BackupManager<P: Platform> {
    platform: P,
    last_backup: Option<SystemTime>,
    backup_interval_seconds: u64,
}

impl<P: Platform> BackupManager<P> {
    pub fn new(platform: P) -> Self {
        Self {
            platform,
            last_backup: None,
            backup_interval_seconds: 300, // 5 minutes
        }
    }

    pub fn with_interval(mut self, interval_seconds: u64) -> Self {
        self.backup_interval_seconds = interval_seconds;
        self
    }

    pub fn should_backup(&self, buffer: &TextBuffer) -> bool {
        if !buffer.is_modified() {
            return false;
        }

        if buffer.file_path().is_none() {
            return false;
        }

        if let Some(last_backup) = self.last_backup {
            if let Ok(elapsed) = last_backup.elapsed() {
                elapsed.as_secs() >= self.backup_interval_seconds
            } else {
                true
            }
        } else {
            true
        }
    }

    pub fn create_backup(&mut self, buffer: &TextBuffer) -> Result<()> {
        let file_path = buffer.file_path()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "No file path set for backup",
            ))?;

        self.ensure_backup_directory()?;
        
        let backup_path = self.generate_backup_path(file_path);
        let content = buffer.to_string();
        
        self.platform.write_file(&backup_path, &content)?;
        self.last_backup = Some(SystemTime::now());
        
        Ok(())
    }

    pub fn create_manual_backup(&mut self, buffer: &TextBuffer) -> Result<()> {
        let file_path = buffer.file_path()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "No file path set for backup",
            ))?;

        self.ensure_backup_directory()?;
        
        let backup_path = self.generate_backup_path(file_path);
        let content = buffer.to_string();
        
        self.platform.write_file(&backup_path, &content)?;
        
        Ok(())
    }

    pub fn list_backups(&self, file_path: &str) -> Result<Vec<BackupInfo>> {
        let filename = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        
        let backup_files = self.platform.list_files(".nowwrite")?;
        let mut backups = Vec::new();
        
        for file in backup_files {
            if file.name.starts_with(&format!("{}.", filename)) && file.name.ends_with(".bak") {
                if let Some(timestamp) = self.extract_timestamp(&file.name) {
                    backups.push(BackupInfo {
                        path: format!(".nowwrite/{}", file.name),
                        timestamp,
                        size: file.size,
                    });
                }
            }
        }
        
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(backups)
    }

    pub fn restore_backup(&mut self, backup_path: &str, target_path: &str) -> Result<()> {
        let content = self.platform.read_file(backup_path)?;
        self.platform.write_file(target_path, &content)?;
        Ok(())
    }

    pub fn cleanup_old_backups(&mut self, file_path: &str, keep_count: usize) -> Result<()> {
        let mut backups = self.list_backups(file_path)?;
        
        if backups.len() > keep_count {
            backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            
            for backup in backups.iter().skip(keep_count) {
                std::fs::remove_file(&backup.path).ok();
            }
        }
        
        Ok(())
    }

    pub fn get_backup_size(&self, file_path: &str) -> Result<u64> {
        let backups = self.list_backups(file_path)?;
        Ok(backups.iter().map(|b| b.size).sum())
    }

    fn ensure_backup_directory(&mut self) -> Result<()> {
        let backup_dir = ".nowwrite";
        if !Path::new(backup_dir).exists() {
            self.platform.create_directory(backup_dir)?;
        }
        Ok(())
    }

    fn generate_backup_path(&self, file_path: &str) -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        format!(".nowwrite/{}.{}.bak", filename, timestamp)
    }

    fn extract_timestamp(&self, backup_filename: &str) -> Option<String> {
        if let Some(start) = backup_filename.rfind('.') {
            if let Some(end) = backup_filename[..start].rfind('.') {
                let timestamp_part = &backup_filename[end + 1..start];
                if self.is_valid_timestamp(timestamp_part) {
                    return Some(timestamp_part.to_string());
                }
            }
        }
        None
    }

    fn is_valid_timestamp(&self, timestamp: &str) -> bool {
        timestamp.len() == 15 && 
        timestamp.chars().nth(8) == Some('_') &&
        timestamp[..8].chars().all(|c| c.is_ascii_digit()) &&
        timestamp[9..].chars().all(|c| c.is_ascii_digit())
    }

    pub fn get_platform_mut(&mut self) -> &mut P {
        &mut self.platform
    }

    pub fn check_and_backup(&mut self, buffer: &TextBuffer) -> Result<bool> {
        if self.should_backup(buffer) {
            self.create_backup(buffer)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: String,
    pub timestamp: String,
    pub size: u64,
}

impl BackupInfo {
    pub fn format_timestamp(&self) -> String {
        if self.timestamp.len() == 15 {
            let date = &self.timestamp[..8];
            let time = &self.timestamp[9..];
            format!("{}-{}-{} {}:{}:{}", 
                   &date[..4], &date[4..6], &date[6..8],
                   &time[..2], &time[2..4], &time[4..6])
        } else {
            self.timestamp.clone()
        }
    }

    pub fn format_size(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        }
    }
}