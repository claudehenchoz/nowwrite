//! ratatui backend that draws its own pixels through the Graphics Output
//! Protocol: 24-bit colour, anti-aliased JetBrains Mono (baked by
//! `build.rs`), real bold and italic, and procedural box drawing.
//!
//! Cells are rendered into a back buffer in RAM; `flush` copies only the
//! changed rows to the screen, one `blt` per dirty rectangle. Using `blt`
//! also works on `BltOnly` firmware and avoids pixel-format handling.

use crate::error::{Error, Result};
use crate::font::{self, boxdraw, Face, Font};
use crate::ui::theme::{self, Rgb};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput, Mode};
use uefi::system;

static FONT_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/font.bin"));

/// Smallest usable screen.
const MIN_RESOLUTION: (usize, usize) = (640, 480);
/// Below this, switch to a larger mode if the firmware offers one...
const SWITCH_BELOW: (usize, usize) = (1024, 768);
/// ...but no larger than this.
const MAX_RESOLUTION: (usize, usize) = (1920, 1200);
/// The font size is the largest that still gives this many cells.
const MIN_GRID: (usize, usize) = (80, 25);

pub struct GopBackend {
    gop: ScopedProtocol<GraphicsOutput>,
    /// The firmware's mode, restored on drop if we switched away from it.
    original: Option<Mode>,
    face: Face<'static>,
    /// Screen size in pixels.
    width: usize,
    height: usize,
    cols: u16,
    rows: u16,
    /// Top-left of the cell grid (the grid is centred on the screen).
    origin: (usize, usize),
    /// Back buffer, `width * height` pixels.
    buffer: Vec<BltPixel>,
    /// Coverage of one procedurally drawn cell.
    scratch: Vec<u8>,
    /// Per cell row, the changed column range `start..end` (empty = clean).
    dirty: Vec<(u16, u16)>,
    cursor: Position,
}

fn area(mode: &Mode) -> usize {
    let (w, h) = mode.info().resolution();
    w * h
}

fn fits(res: (usize, usize), min: (usize, usize)) -> bool {
    res.0 >= min.0 && res.1 >= min.1
}

fn pixel((r, g, b): Rgb) -> BltPixel {
    BltPixel::new(r, g, b)
}

/// `bg + (fg - bg) * a / 255`, per channel.
fn blend(fg: BltPixel, bg: BltPixel, a: u8) -> BltPixel {
    let mix = |f: u8, b: u8| {
        let (f, b, a) = (i32::from(f), i32::from(b), i32::from(a));
        (b + (f - b) * a / 255) as u8
    };
    BltPixel::new(mix(fg.red, bg.red), mix(fg.green, bg.green), mix(fg.blue, bg.blue))
}

impl GopBackend {
    /// `None` if there is no GOP, no mode of at least 640x480, or no font.
    pub fn new() -> Option<Self> {
        let font = Font::new(FONT_DATA)?;
        let handle = boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
        // Non-exclusive, so the firmware's text console keeps working for
        // the error message `uefi_main` prints after the editor exits.
        // SAFETY: we're the only user of GOP while the editor runs, and the
        // console handle isn't uninstalled while boot services run.
        let mut gop = unsafe {
            boot::open_protocol::<GraphicsOutput>(
                OpenProtocolParams { handle, agent: boot::image_handle(), controller: None },
                OpenProtocolAttributes::GetProtocol,
            )
        }
        .ok()?;

        let modes: Vec<Mode> = gop.modes().collect();
        if !modes.iter().any(|m| fits(m.info().resolution(), MIN_RESOLUTION)) {
            return None;
        }
        let current_info = gop.current_mode_info();
        let current = current_info.resolution();
        let mut original = None;
        if !fits(current, SWITCH_BELOW) {
            let best = modes
                .iter()
                .filter(|m| fits(MAX_RESOLUTION, m.info().resolution()))
                .max_by_key(|m| area(m));
            if let Some(best) = best.filter(|m| area(m) > current.0 * current.1) {
                // If the switch fails, the current mode still works.
                if gop.set_mode(best).is_ok() {
                    original = modes.iter().copied().find(|m| *m.info() == current_info);
                }
            }
        }
        let (width, height) = gop.current_mode_info().resolution();
        if !fits((width, height), MIN_RESOLUTION) {
            return None;
        }

        // Largest face giving at least 80x25 cells, else the smallest.
        let face = font
            .faces()
            .filter(|f| {
                fits((width / usize::from(f.cell_w), height / usize::from(f.cell_h)), MIN_GRID)
            })
            .last()
            .or_else(|| font.face(0))?;
        let (cw, ch) = (usize::from(face.cell_w), usize::from(face.cell_h));
        let (cols, rows) = (width / cw, height / ch);

        // The firmware console draws its cursor through GOP; keep it hidden.
        let _ = system::with_stdout(|out| out.enable_cursor(false));

        let mut backend = Self {
            gop,
            original,
            face,
            width,
            height,
            cols: cols as u16,
            rows: rows as u16,
            origin: ((width - cols * cw) / 2, (height - rows * ch) / 2),
            buffer: vec![BltPixel::new(0, 0, 0); width * height],
            scratch: vec![0; cw * ch],
            dirty: vec![(0, 0); rows],
            cursor: Position::ORIGIN,
        };
        backend.clear().ok()?;
        Some(backend)
    }

    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: BltPixel) {
        for row in y..(y + h).min(self.height) {
            let start = row * self.width + x;
            let end = row * self.width + (x + w).min(self.width);
            self.buffer[start..end].fill(color);
        }
    }

    /// Blend `coverage` (`w` x `h`, row stride `w`) at pixel `x`, `y`.
    fn blend_coverage(&mut self, x: usize, y: usize, w: usize, h: usize, coverage: &[u8], fg: BltPixel, bg: BltPixel) {
        for gy in 0..h {
            let row = (y + gy) * self.width + x;
            for gx in 0..w {
                let a = coverage[gy * w + gx];
                if a != 0 {
                    self.buffer[row + gx] = if a == 255 { fg } else { blend(fg, bg, a) };
                }
            }
        }
    }

    fn draw_cell(&mut self, x: u16, y: u16, cell: &Cell) {
        if x >= self.cols || y >= self.rows {
            return;
        }
        let face = self.face;
        let (cw, ch) = (usize::from(face.cell_w), usize::from(face.cell_h));
        let (px, py) = self.cell_origin(x, y, false);

        let t = theme::current();
        let mut fg = pixel(theme::to_rgb(cell.fg, t.text));
        let mut bg = pixel(theme::to_rgb(cell.bg, t.background));
        if cell.modifier.contains(Modifier::REVERSED) {
            core::mem::swap(&mut fg, &mut bg);
        }
        // Cells on the grid's edges also paint the margin next to them, so
        // e.g. the status bar reaches the screen edges.
        let (left, top) = self.cell_origin(x, y, true);
        let (right, bottom) = self.cell_origin(x + 1, y + 1, true);
        self.fill(left, top, right - left, bottom - top, bg);

        // Combining marks and wide symbols: draw the first character only.
        let symbol = cell.symbol().chars().next().unwrap_or(' ');
        let thick = boxdraw::thickness(face.px);
        if symbol != ' ' {
            let mut style = 0;
            if cell.modifier.contains(Modifier::BOLD) {
                style |= font::BOLD;
            }
            if cell.modifier.contains(Modifier::ITALIC) {
                style |= font::ITALIC;
            }
            if boxdraw::render(symbol, cw, ch, thick, &mut self.scratch) {
                let scratch = core::mem::take(&mut self.scratch);
                self.blend_coverage(px, py, cw, ch, &scratch, fg, bg);
                self.scratch = scratch;
            } else if let Some(g) = face.glyph(symbol, style) {
                self.blend_coverage(px + usize::from(g.x), py + usize::from(g.y), usize::from(g.w), usize::from(g.h), g.coverage, fg, bg);
            } else {
                // Missing glyph: a hollow box from cap height to baseline.
                let (bx, bw) = (px + cw / 5, cw - 2 * (cw / 5));
                let top = py + usize::from(face.baseline).saturating_sub(usize::from(face.px) * 7 / 10);
                let bottom = py + usize::from(face.baseline);
                self.fill(bx, top, bw, thick, fg);
                self.fill(bx, bottom - thick, bw, thick, fg);
                self.fill(bx, top, thick, bottom - top, fg);
                self.fill(bx + bw - thick, top, thick, bottom - top, fg);
            }
        }

        let baseline = py + usize::from(face.baseline);
        if cell.modifier.contains(Modifier::UNDERLINED) {
            let y = (baseline + thick).min(py + ch - thick);
            self.fill(px, y, cw, thick, fg);
        }
        if cell.modifier.contains(Modifier::CROSSED_OUT) {
            self.fill(px, baseline - usize::from(face.px) * 3 / 10, cw, thick, fg);
        }

        let (start, end) = &mut self.dirty[usize::from(y)];
        if start == end {
            (*start, *end) = (x, x + 1);
        } else {
            (*start, *end) = ((*start).min(x), (*end).max(x + 1));
        }
    }

    /// Pixel position of the top-left of cell `x`, `y`. With `margins`,
    /// the first row/column starts at the screen edge, and one past the
    /// last row/column is the screen edge.
    fn cell_origin(&self, x: u16, y: u16, margins: bool) -> (usize, usize) {
        let (cw, ch) = (usize::from(self.face.cell_w), usize::from(self.face.cell_h));
        let edge = |i: u16, count: u16, origin: usize, size: usize, full: usize| match i {
            0 if margins => 0,
            i if margins && i >= count => full,
            i => origin + usize::from(i) * size,
        };
        (
            edge(x, self.cols, self.origin.0, cw, self.width),
            edge(y, self.rows, self.origin.1, ch, self.height),
        )
    }

    /// Copy a rectangle of the back buffer to the screen.
    fn blit(&mut self, x: usize, y: usize, w: usize, h: usize) -> Result<()> {
        if w == 0 || h == 0 {
            return Ok(());
        }
        self.gop
            .blt(BltOp::BufferToVideo {
                buffer: &self.buffer,
                src: BltRegion::SubRectangle { coords: (x, y), px_stride: self.width },
                dest: (x, y),
                dims: (w, h),
            })
            .map_err(|e| Error::Io(format!("GOP blt: {e:?}")))
    }
}

impl Drop for GopBackend {
    /// Give the firmware its resolution back: its text console keeps
    /// drawing for the old one, e.g. the error message after a failure.
    fn drop(&mut self) {
        if let Some(mode) = self.original.take() {
            let _ = self.gop.set_mode(&mode);
        }
    }
}

impl Backend for GopBackend {
    type Error = Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            self.draw_cell(x, y, cell);
        }
        Ok(())
    }

    /// The editor draws its own cursor (`Platform::SOFTWARE_CURSOR`).
    fn hide_cursor(&mut self) -> Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<()> {
        self.cursor = position.into();
        Ok(())
    }

    /// Fill the whole screen, margins included, with the theme background.
    fn clear(&mut self) -> Result<()> {
        let bg = pixel(theme::to_rgb(theme::current().background, Color::Reset));
        self.buffer.fill(bg);
        self.dirty.fill((0, 0));
        self.blit(0, 0, self.width, self.height)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<()> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(Error::Unsupported),
        }
    }

    fn size(&self) -> Result<Size> {
        Ok(Size::new(self.cols, self.rows))
    }

    fn window_size(&mut self) -> Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: Size::new(self.cols, self.rows),
            pixels: Size::new(self.width as u16, self.height as u16),
        })
    }

    /// Copy changed rows to the screen. Consecutive dirty rows merge into
    /// one rectangle spanning their combined columns.
    fn flush(&mut self) -> Result<()> {
        let mut row = 0;
        while row < self.dirty.len() {
            let (mut start, mut end) = self.dirty[row];
            if start == end {
                row += 1;
                continue;
            }
            let first = row;
            row += 1;
            while row < self.dirty.len() && self.dirty[row].0 != self.dirty[row].1 {
                start = start.min(self.dirty[row].0);
                end = end.max(self.dirty[row].1);
                row += 1;
            }
            let (x0, y0) = self.cell_origin(start, first as u16, true);
            let (x1, y1) = self.cell_origin(end, row as u16, true);
            self.blit(x0, y0, x1 - x0, y1 - y0)?;
        }
        self.dirty.fill((0, 0));
        Ok(())
    }
}
