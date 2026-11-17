pub mod backup;
pub mod manager;

/// Folder (relative to the document root) holding config and backups.
pub const NOWWRITE_DIR: &str = ".nowwrite";

/// Last component of a `/`- or `\`-separated path.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_name_strips_dirs() {
        assert_eq!(super::file_name("notes.md"), "notes.md");
        assert_eq!(super::file_name("./a/notes.md"), "notes.md");
        assert_eq!(super::file_name(r"\a\notes.md"), "notes.md");
    }
}
