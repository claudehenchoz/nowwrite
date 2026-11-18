//! Writes the baked font blob read by `font::Font`. Shared with `build.rs`
//! (via `#[path]`) and the reader's tests, so the format lives in one place.
//!
//! Layout, all integers little-endian:
//! ```text
//! "NWF1"  u32 size_count
//! size_count x 16 bytes: u16 px, u16 cell_w, u16 cell_h, u16 baseline,
//!                        u32 glyph_count, u32 table_offset
//! per size, glyph_count x 16 bytes, sorted by (char, style):
//!     u32 char, u8 style, u8 x, u8 y, u8 w, u8 h, 3 x u8 padding, u32 offset
//! coverage bytes: w * h per glyph, row-major, 0 = background, 255 = ink
//! ```
//! `x`/`y` place the glyph's tight bounding box inside its cell.

use alloc::vec::Vec;

pub const MAGIC: &[u8; 4] = b"NWF1";
pub const HEADER_LEN: usize = 8;
pub const SIZE_RECORD_LEN: usize = 16;
pub const GLYPH_RECORD_LEN: usize = 16;

pub struct BakedGlyph {
    pub ch: u32,
    /// Bit 0 bold, bit 1 italic.
    pub style: u8,
    pub x: u8,
    pub y: u8,
    pub w: u8,
    pub h: u8,
    pub coverage: Vec<u8>,
}

pub struct BakedSize {
    pub px: u16,
    pub cell_w: u16,
    pub cell_h: u16,
    pub baseline: u16,
    pub glyphs: Vec<BakedGlyph>,
}

pub fn write(sizes: &mut [BakedSize]) -> Vec<u8> {
    let tables_start = HEADER_LEN + sizes.len() * SIZE_RECORD_LEN;
    let glyph_total: usize = sizes.iter().map(|s| s.glyphs.len()).sum();
    let mut coverage_offset = tables_start + glyph_total * GLYPH_RECORD_LEN;

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(sizes.len() as u32).to_le_bytes());
    let mut table_offset = tables_start;
    for size in sizes.iter_mut() {
        size.glyphs.sort_by_key(|g| (g.ch, g.style));
        for v in [size.px, size.cell_w, size.cell_h, size.baseline] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(size.glyphs.len() as u32).to_le_bytes());
        out.extend_from_slice(&(table_offset as u32).to_le_bytes());
        table_offset += size.glyphs.len() * GLYPH_RECORD_LEN;
    }
    for size in sizes.iter() {
        for g in &size.glyphs {
            out.extend_from_slice(&g.ch.to_le_bytes());
            out.extend_from_slice(&[g.style, g.x, g.y, g.w, g.h, 0, 0, 0]);
            out.extend_from_slice(&(coverage_offset as u32).to_le_bytes());
            coverage_offset += g.coverage.len();
        }
    }
    for size in sizes.iter() {
        for g in &size.glyphs {
            out.extend_from_slice(&g.coverage);
        }
    }
    out
}
