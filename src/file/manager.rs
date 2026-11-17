use crate::editor::buffer::TextBuffer;
use crate::error::{Error, Result};
use crate::platform::{FileInfo, Platform};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Directory whose `.md` files are offered for editing (the boot volume's
/// root on UEFI, the working directory in a terminal).
pub const DOCUMENT_DIR: &str = ".";

pub fn list_documents<P: Platform>(platform: &mut P) -> Result<Vec<FileInfo>> {
    let mut files = platform.list_files(DOCUMENT_DIR)?;
    files.retain(|f| !f.is_directory && f.name.ends_with(".md"));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

pub fn load<P: Platform>(platform: &mut P, path: &str) -> Result<TextBuffer> {
    let content = platform.read_file(path)?;
    let mut buffer = TextBuffer::from_string(&content);
    buffer.set_file_path(path.to_string());
    buffer.clear_modified();
    Ok(buffer)
}

pub fn save<P: Platform>(platform: &mut P, buffer: &mut TextBuffer) -> Result<()> {
    let path = buffer.file_path().ok_or(Error::NotFound)?.to_string();
    platform.write_file(&path, &buffer.to_text())?;
    buffer.clear_modified();
    Ok(())
}

/// A file name for a new document that doesn't exist yet:
/// `untitled.md`, `untitled_1.md`, ...
pub fn unused_name<P: Platform>(platform: &mut P, base: &str) -> String {
    let base: String = base
        .trim_end_matches(".md")
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();

    let mut name = format!("{base}.md");
    let mut counter = 1;
    while platform.exists(&name) {
        name = format!("{base}_{counter}.md");
        counter += 1;
    }
    name
}
