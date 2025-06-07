mod editor;
mod ui;
mod file;
mod platform;
mod config;

use std::io::Result;

#[cfg(feature = "linux")]
use platform::linux::LinuxPlatform;
#[cfg(feature = "uefi")]
use platform::uefi::UefiPlatform;

use platform::Platform;
use editor::Editor;

fn main() -> Result<()> {
    #[cfg(feature = "linux")]
    let platform = LinuxPlatform::init()?;
    
    #[cfg(feature = "uefi")]
    let platform = UefiPlatform::init()?;
    
    let mut editor = Editor::new(platform)?;
    editor.run()
}