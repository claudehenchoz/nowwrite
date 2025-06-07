use crate::platform::{Platform, FileInfo};
use crate::editor::buffer::TextBuffer;
use std::io::Result;
use std::path::Path;

pub struct FileManager<P: Platform> {
    platform: P,
    current_directory: String,
}

impl<P: Platform> FileManager<P> {
    pub fn new(platform: P) -> Self {
        Self {
            platform,
            current_directory: ".".to_string(),
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<TextBuffer> {
        let content = self.platform.read_file(path)?;
        let mut buffer = TextBuffer::from_string(&content);
        buffer.set_file_path(path.to_string());
        buffer.clear_modified();
        Ok(buffer)
    }

    pub fn save_file(&mut self, buffer: &mut TextBuffer) -> Result<()> {
        let path = if let Some(file_path) = buffer.file_path() {
            file_path.clone()
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "No file path set",
            ));
        };

        self.create_backup_if_exists(&path)?;
        
        let content = buffer.to_string();
        self.platform.write_file(&path, &content)?;
        buffer.clear_modified();
        
        Ok(())
    }

    pub fn save_file_as(&mut self, buffer: &mut TextBuffer, path: &str) -> Result<()> {
        self.create_backup_if_exists(path)?;
        
        let content = buffer.to_string();
        self.platform.write_file(path, &content)?;
        buffer.set_file_path(path.to_string());
        buffer.clear_modified();
        
        Ok(())
    }

    pub fn create_new_file(&mut self, path: &str) -> Result<TextBuffer> {
        if Path::new(path).exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "File already exists",
            ));
        }

        let mut buffer = TextBuffer::new();
        buffer.set_file_path(path.to_string());
        
        self.platform.write_file(path, "")?;
        buffer.clear_modified();
        
        Ok(buffer)
    }

    pub fn list_markdown_files(&self) -> Result<Vec<FileInfo>> {
        let mut files = self.platform.list_files(&self.current_directory)?;
        
        files.retain(|file| {
            file.is_directory || file.name.ends_with(".md")
        });
        
        files.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        Ok(files)
    }

    pub fn ensure_nowwrite_directory(&mut self) -> Result<()> {
        let nowwrite_dir = ".nowwrite";
        if !Path::new(nowwrite_dir).exists() {
            self.platform.create_directory(nowwrite_dir)?;
        }
        Ok(())
    }

    pub fn get_backup_path(&self, original_path: &str) -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = Path::new(original_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        format!(".nowwrite/{}.{}.bak", filename, timestamp)
    }

    fn create_backup_if_exists(&mut self, path: &str) -> Result<()> {
        if Path::new(path).exists() {
            let backup_path = self.get_backup_path(path);
            let content = self.platform.read_file(path)?;
            self.ensure_nowwrite_directory()?;
            self.platform.write_file(&backup_path, &content)?;
        }
        Ok(())
    }

    pub fn get_platform_mut(&mut self) -> &mut P {
        &mut self.platform
    }

    pub fn change_directory(&mut self, path: &str) -> Result<()> {
        self.current_directory = path.to_string();
        Ok(())
    }

    pub fn current_directory(&self) -> &str {
        &self.current_directory
    }

    pub fn delete_file(&mut self, path: &str) -> Result<()> {
        self.create_backup_if_exists(path)?;
        std::fs::remove_file(path)?;
        Ok(())
    }

    pub fn file_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn get_file_size(&self, path: &str) -> Result<u64> {
        let metadata = std::fs::metadata(path)?;
        Ok(metadata.len())
    }

    pub fn get_safe_filename(&self, desired_name: &str) -> String {
        let mut name = desired_name.to_string();
        
        if !name.ends_with(".md") {
            name.push_str(".md");
        }
        
        name = name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c => c,
            })
            .collect();

        let mut counter = 1;
        let mut final_name = name.clone();
        
        while self.file_exists(&final_name) {
            let base = if name.ends_with(".md") {
                &name[..name.len() - 3]
            } else {
                &name
            };
            final_name = format!("{}_{}.md", base, counter);
            counter += 1;
        }
        
        final_name
    }
}