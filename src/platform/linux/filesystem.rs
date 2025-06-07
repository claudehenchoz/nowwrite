use crate::platform::FileInfo;
use std::fs;
use std::io::Result;
use std::path::Path;

pub struct LinuxFileSystem;

impl LinuxFileSystem {
    pub fn list_files(path: &str) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();
            
            // Only include directories and .md files
            if metadata.is_dir() || name.ends_with(".md") {
                files.push(FileInfo {
                    name,
                    is_directory: metadata.is_dir(),
                    size: metadata.len(),
                });
            }
        }
        
        // Sort directories first, then by name
        files.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        Ok(files)
    }

    pub fn read_file(path: &str) -> Result<String> {
        fs::read_to_string(path)
    }

    pub fn write_file(path: &str, content: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
    }

    pub fn create_directory(path: &str) -> Result<()> {
        fs::create_dir_all(path)
    }

    pub fn delete_file(path: &str) -> Result<()> {
        fs::remove_file(path)
    }

    pub fn file_exists(path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn get_file_size(path: &str) -> Result<u64> {
        let metadata = fs::metadata(path)?;
        Ok(metadata.len())
    }

    pub fn copy_file(src: &str, dst: &str) -> Result<()> {
        fs::copy(src, dst)?;
        Ok(())
    }

    pub fn move_file(src: &str, dst: &str) -> Result<()> {
        fs::rename(src, dst)
    }

    pub fn get_current_dir() -> Result<String> {
        let current_dir = std::env::current_dir()?;
        Ok(current_dir.to_string_lossy().to_string())
    }

    pub fn change_dir(path: &str) -> Result<()> {
        std::env::set_current_dir(path)?;
        Ok(())
    }

    pub fn is_absolute_path(path: &str) -> bool {
        Path::new(path).is_absolute()
    }

    pub fn join_path(base: &str, relative: &str) -> String {
        Path::new(base).join(relative).to_string_lossy().to_string()
    }

    pub fn get_parent_dir(path: &str) -> Option<String> {
        Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
    }

    pub fn get_filename(path: &str) -> Option<String> {
        Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
    }

    pub fn get_file_extension(path: &str) -> Option<String> {
        Path::new(path)
            .extension()
            .map(|ext| ext.to_string_lossy().to_string())
    }
}