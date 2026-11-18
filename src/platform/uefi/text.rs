//! ratatui backend over the UEFI Simple Text Output protocol (the fallback
//! when graphics mode is unavailable or disabled).

use crate::error::{Error, Result};
use crate::ui::theme;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use uefi::proto::console::text::Color;
use uefi::{system, CString16};

fn uefi_err(err: uefi::Error) -> Error {
    Error::Io(format!("{err:?}"))
}

pub struct TextBackend {
    size: Size,
    cursor: Position,
}

impl TextBackend {
    /// Switch to the text mode with the most cells and hide the firmware cursor.
    pub fn new() -> Result<Self> {
        system::with_stdout(|out| {
            let modes: Vec<_> = out.modes().collect();
            if let Some(best) = modes.into_iter().max_by_key(|m| m.columns() * m.rows()) {
                // Some firmware rejects modes it lists; the current one still works.
                let _ = out.set_mode(best);
            }
            // The editor draws its own cursor (Platform::SOFTWARE_CURSOR).
            let _ = out.enable_cursor(false);
            let (fg, bg) = base_colors();
            out.set_color(fg, bg).map_err(uefi_err)?;
            out.clear().map_err(uefi_err)?;
            let mode = out
                .current_mode()
                .map_err(uefi_err)?
                .ok_or_else(|| Error::Io(String::from("no text mode")))?;
            Ok(Self {
                size: Size::new(mode.columns() as u16, mode.rows() as u16),
                cursor: Position::ORIGIN,
            })
        })
    }
}

/// UEFI console colours in attribute order (index = VGA palette index).
const CONSOLE: [Color; 16] = [
    Color::Black, Color::Blue, Color::Green, Color::Cyan,
    Color::Red, Color::Magenta, Color::Brown, Color::LightGray,
    Color::DarkGray, Color::LightBlue, Color::LightGreen, Color::LightCyan,
    Color::LightRed, Color::LightMagenta, Color::Yellow, Color::White,
];

/// Map a cell's theme colours onto the console's 16 foreground / 8
/// background colours (see `theme::console_colors`).
fn colors(cell: &Cell) -> (Color, Color) {
    let (fg, bg) = theme::console_colors(cell.fg, cell.bg, theme::current());
    (CONSOLE[usize::from(fg)], CONSOLE[usize::from(bg & 7)])
}

/// The theme's plain text colours, for clearing the screen.
fn base_colors() -> (Color, Color) {
    let t = theme::current();
    let (fg, bg) = theme::console_colors(t.text, t.background, t);
    (CONSOLE[usize::from(fg)], CONSOLE[usize::from(bg & 7)])
}

/// UEFI text output is UCS-2: replace anything outside the BMP. The
/// firmware font lacks rounded corners, so use square ones.
fn push_ucs2(out: &mut String, symbol: &str) {
    for ch in symbol.chars() {
        out.push(match ch {
            '╭' => '┌',
            '╮' => '┐',
            '╰' => '└',
            '╯' => '┘',
            c if (c as u32) <= 0xFFFF && !c.is_control() => c,
            _ => '?',
        });
    }
}

impl Backend for TextBackend {
    type Error = Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let (width, height) = (self.size.width, self.size.height);
        // Batch consecutive same-colored cells on a row into one output call.
        let mut run = String::new();
        let mut run_start = (0u16, 0u16);
        let mut run_colors = (Color::LightGray, Color::Black);
        let mut next_x = None;

        let flush = |run: &mut String, start: (u16, u16), colors: (Color, Color)| -> Result<()> {
            if run.is_empty() {
                return Ok(());
            }
            let text = CString16::try_from(run.as_str()).map_err(|e| Error::Io(format!("{e:?}")))?;
            system::with_stdout(|out| -> Result<()> {
                let at = |op: &str, e: uefi::Error| Error::Io(format!("{op} at {start:?}: {e:?}"));
                move_cursor(out, start.0, start.1);
                out.set_color(colors.0, colors.1).map_err(|e| at("set_color", e))?;
                // Warnings such as WARN_UNKNOWN_GLYPH still draw the rest of the text.
                match out.output_string(&text) {
                    Err(e) if !e.status().is_warning() => Err(at("output_string", e)),
                    _ => Ok(()),
                }
            })?;
            run.clear();
            Ok(())
        };

        for (x, y, cell) in content {
            // Writing the bottom-right cell makes the console scroll.
            if x + 1 >= width && y + 1 >= height {
                continue;
            }
            let symbol = cell.symbol();
            if symbol.is_empty() {
                continue; // trailing half of a wide char
            }
            let colors = colors(cell);
            let contiguous = next_x == Some((x, y)) && colors_eq(colors, run_colors);
            if !contiguous {
                flush(&mut run, run_start, run_colors)?;
                run_start = (x, y);
                run_colors = colors;
            }
            push_ucs2(&mut run, symbol);
            next_x = Some((x + 1, y));
        }
        flush(&mut run, run_start, run_colors)?;
        // Output moved the hardware cursor; ratatui repositions it after draw.
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<()> {
        let _ = system::with_stdout(|out| out.enable_cursor(false));
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<()> {
        let _ = system::with_stdout(|out| out.enable_cursor(true));
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<()> {
        let position = position.into();
        system::with_stdout(|out| move_cursor(out, position.x, position.y));
        self.cursor = position;
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        let (fg, bg) = base_colors();
        system::with_stdout(|out| {
            out.set_color(fg, bg)?;
            out.clear()
        })
        .map_err(uefi_err)?;
        self.cursor = Position::ORIGIN;
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<()> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(Error::Unsupported),
        }
    }

    fn size(&self) -> Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> Result<WindowSize> {
        Ok(WindowSize { columns_rows: self.size, pixels: Size::new(0, 0) })
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Position the cursor, ignoring errors. The firmware's console splitter
/// reports UNSUPPORTED when any attached console (e.g. a narrower serial
/// terminal) can't reach the position, even though the screen itself moved.
fn move_cursor(out: &mut uefi::proto::console::text::Output, x: u16, y: u16) {
    let _ = out.set_cursor_position(usize::from(x), usize::from(y));
}

fn colors_eq(a: (Color, Color), b: (Color, Color)) -> bool {
    a.0 as u8 == b.0 as u8 && a.1 as u8 == b.1 as u8
}
