//! NowWrite core: editor, UI and file handling, independent of the platform.
//!
//! This crate is `no_std` + `alloc` so the same code runs as a UEFI
//! application. Only `platform::terminal` (feature `terminal`) may use `std`.

#![cfg_attr(not(test), no_std)]

extern crate alloc;
#[cfg(any(test, feature = "terminal"))]
extern crate std;

pub mod config;
pub mod editor;
pub mod error;
pub mod file;
pub mod font;
pub mod heap;
pub mod perf;
pub mod platform;
pub mod ui;

pub use error::{Error, Result};

use platform::Platform;

/// Run the editor until the user exits. Consumes the platform; terminal
/// state is restored when it is dropped.
pub fn run<P: Platform>(platform: P, backend: P::Backend) -> Result<()> {
    let mut editor = editor::Editor::new(platform, backend)?;
    editor.run()
}
