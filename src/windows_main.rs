mod editor;
mod ui;
mod file;
mod platform;
mod config;

use std::io::Result;

use platform::windows::WindowsPlatform;
use platform::Platform;
use editor::Editor;

fn main() -> Result<()> {
    let platform = WindowsPlatform::init()?;
    let mut editor = Editor::new(platform)?;
    editor.run()
}
