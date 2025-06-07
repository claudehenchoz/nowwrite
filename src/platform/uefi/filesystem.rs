#[cfg(feature = "uefi")]
use uefi::proto::media::fs::SimpleFileSystem;
#[cfg(feature = "uefi")]
use uefi::proto::media::file::{File, FileAttribute, FileMode, FileType};
#[cfg(feature = "uefi")]
use uefi::CStr16;

use crate::platform::FileInfo;
use std::io::Result;

pub struct UefiFileSystem;

impl UefiFileSystem {
    #[cfg(feature = "uefi")]
    pub fn list_files(fs: &mut SimpleFileSystem, path: &str) -> Result<Vec<FileInfo>> {
        let mut root = fs.open_volume().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let path_utf16 = Self::str_to_utf16(path);
        let mut dir = if path == "." || path == "/" {
            root
        } else {
            root.open(&path_utf16, FileMode::Read, FileAttribute::empty())
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
                })?
                .into_directory()
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not a directory")
                })?
        };

        let mut files = Vec::new();
        let mut buffer = [0u8; 512];

        loop {
            let info = match dir.read_entry(&mut buffer) {
                Ok(Some(info)) => info,
                Ok(None) => break,
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("UEFI error: {:?}", e),
                    ))
                }
            };

            let name = Self::utf16_to_string(info.file_name());
            if name == "." || name == ".." {
                continue;
            }

            let is_directory = info.attribute().contains(FileAttribute::DIRECTORY);
            let size = info.file_size();

            if is_directory || name.ends_with(".md") {
                files.push(FileInfo {
                    name,
                    is_directory,
                    size,
                });
            }
        }

        Ok(files)
    }

    #[cfg(not(feature = "uefi"))]
    pub fn list_files(_fs: &mut (), _path: &str) -> Result<Vec<FileInfo>> {
        Ok(Vec::new())
    }

    #[cfg(feature = "uefi")]
    pub fn read_file(fs: &mut SimpleFileSystem, path: &str) -> Result<String> {
        let mut root = fs.open_volume().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let path_utf16 = Self::str_to_utf16(path);
        let mut file = root
            .open(&path_utf16, FileMode::Read, FileAttribute::empty())
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
            })?
            .into_regular_file()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not a regular file")
            })?;

        // Get file size
        let mut buffer = [0u8; 512];
        let info = file.get_info::<uefi::proto::media::file::FileInfo>(&mut buffer)
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
            })?;

        let file_size = info.file_size() as usize;
        let mut content = vec![0u8; file_size];

        file.read(&mut content).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        String::from_utf8(content).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("UTF-8 error: {}", e))
        })
    }

    #[cfg(not(feature = "uefi"))]
    pub fn read_file(_fs: &mut (), _path: &str) -> Result<String> {
        Ok(String::new())
    }

    #[cfg(feature = "uefi")]
    pub fn write_file(fs: &mut SimpleFileSystem, path: &str, content: &str) -> Result<()> {
        let mut root = fs.open_volume().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let path_utf16 = Self::str_to_utf16(path);
        let mut file = root
            .open(
                &path_utf16,
                FileMode::CreateReadWrite,
                FileAttribute::empty(),
            )
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
            })?
            .into_regular_file()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Could not create file")
            })?;

        let content_bytes = content.as_bytes();
        file.write(content_bytes).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        file.flush().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        Ok(())
    }

    #[cfg(not(feature = "uefi"))]
    pub fn write_file(_fs: &mut (), _path: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn create_directory(fs: &mut SimpleFileSystem, path: &str) -> Result<()> {
        let mut root = fs.open_volume().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let path_utf16 = Self::str_to_utf16(path);
        root.open(
            &path_utf16,
            FileMode::CreateReadWrite,
            FileAttribute::DIRECTORY,
        )
        .map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        Ok(())
    }

    #[cfg(not(feature = "uefi"))]
    pub fn create_directory(_fs: &mut (), _path: &str) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "uefi")]
    fn str_to_utf16(s: &str) -> uefi::CStr16 {
        use uefi::CStr16;
        
        let mut utf16_vec: Vec<u16> = s.encode_utf16().collect();
        utf16_vec.push(0); // Null terminator
        
        // This is unsafe but necessary for UEFI
        // In a real implementation, you'd want proper error handling
        unsafe { CStr16::from_u16_with_nul_unchecked(&utf16_vec) }
    }

    #[cfg(feature = "uefi")]
    fn utf16_to_string(cstr16: &CStr16) -> String {
        let utf16_slice = cstr16.as_slice();
        String::from_utf16_lossy(utf16_slice)
    }

    #[cfg(feature = "uefi")]
    pub fn delete_file(fs: &mut SimpleFileSystem, path: &str) -> Result<()> {
        let mut root = fs.open_volume().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        let path_utf16 = Self::str_to_utf16(path);
        let mut file = root
            .open(&path_utf16, FileMode::Read, FileAttribute::empty())
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
            })?;

        file.delete().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("UEFI error: {:?}", e))
        })?;

        Ok(())
    }

    #[cfg(feature = "uefi")]
    pub fn file_exists(fs: &mut SimpleFileSystem, path: &str) -> bool {
        let Ok(mut root) = fs.open_volume() else {
            return false;
        };

        let path_utf16 = Self::str_to_utf16(path);
        root.open(&path_utf16, FileMode::Read, FileAttribute::empty())
            .is_ok()
    }
}