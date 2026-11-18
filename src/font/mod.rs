//! Reader for the bitmap font baked by `build.rs` (JetBrains Mono, several
//! pixel sizes, four styles). Lookups binary-search the blob in place; there
//! is no parsing beyond reading fixed-size records.

pub mod boxdraw;
#[cfg(test)]
mod writer;

/// Glyph style bits.
pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;

const MAGIC: &[u8; 4] = b"NWF1";
const HEADER_LEN: usize = 8;
const SIZE_RECORD_LEN: usize = 16;
const GLYPH_RECORD_LEN: usize = 16;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// The whole baked font: one `Face` per pixel size.
#[derive(Clone, Copy)]
pub struct Font<'a> {
    data: &'a [u8],
    count: usize,
}

impl<'a> Font<'a> {
    /// `None` for an empty or malformed blob (e.g. a non-UEFI build).
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.get(..4)? != MAGIC {
            return None;
        }
        let count = u32_at(data, 4)? as usize;
        data.get(..HEADER_LEN + count * SIZE_RECORD_LEN)?;
        Some(Self { data, count })
    }

    /// Faces in the order they were baked (ascending size).
    pub fn faces(&self) -> impl Iterator<Item = Face<'a>> + '_ {
        (0..self.count).filter_map(|i| self.face(i))
    }

    pub fn face(&self, index: usize) -> Option<Face<'a>> {
        if index >= self.count {
            return None;
        }
        let at = HEADER_LEN + index * SIZE_RECORD_LEN;
        let d = self.data;
        let count = u32_at(d, at + 8)? as usize;
        let start = u32_at(d, at + 12)? as usize;
        let table = d.get(start..start + count * GLYPH_RECORD_LEN)?;
        Some(Face {
            px: u16_at(d, at)?,
            cell_w: u16_at(d, at + 2)?,
            cell_h: u16_at(d, at + 4)?,
            baseline: u16_at(d, at + 6)?,
            table,
            data: d,
        })
    }
}

/// One pixel size: cell metrics and its glyph table.
#[derive(Clone, Copy)]
pub struct Face<'a> {
    pub px: u16,
    pub cell_w: u16,
    pub cell_h: u16,
    /// Baseline, in pixels from the top of the cell.
    pub baseline: u16,
    table: &'a [u8],
    data: &'a [u8],
}

/// A glyph's anti-aliased coverage, placed at `x`, `y` inside the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph<'a> {
    pub x: u8,
    pub y: u8,
    pub w: u8,
    pub h: u8,
    /// `w * h` bytes, row-major, 0..=255.
    pub coverage: &'a [u8],
}

impl<'a> Face<'a> {
    pub fn glyph_count(&self) -> usize {
        self.table.len() / GLYPH_RECORD_LEN
    }

    fn key(&self, index: usize) -> (u32, u8) {
        let at = index * GLYPH_RECORD_LEN;
        (u32_at(self.table, at).unwrap_or(0), self.table[at + 4])
    }

    /// The glyph for `ch` in `style` (`BOLD | ITALIC` bits), falling back
    /// to the regular style. `None` if the font lacks the character.
    pub fn glyph(&self, ch: char, style: u8) -> Option<Glyph<'a>> {
        self.find(ch as u32, style & (BOLD | ITALIC)).or_else(|| self.find(ch as u32, 0))
    }

    fn find(&self, ch: u32, style: u8) -> Option<Glyph<'a>> {
        let (mut lo, mut hi) = (0, self.glyph_count());
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.key(mid).cmp(&(ch, style)) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => {
                    let r = &self.table[mid * GLYPH_RECORD_LEN..(mid + 1) * GLYPH_RECORD_LEN];
                    let (w, h) = (r[7], r[8]);
                    let offset = u32_at(r, 12)? as usize;
                    let coverage = self.data.get(offset..offset + usize::from(w) * usize::from(h))?;
                    return Some(Glyph { x: r[5], y: r[6], w, h, coverage });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::writer::{write, BakedGlyph, BakedSize, MAGIC as WRITER_MAGIC};
    use super::*;
    use alloc::vec;

    fn glyph(ch: char, style: u8, w: u8, h: u8, fill: u8) -> BakedGlyph {
        BakedGlyph { ch: ch as u32, style, x: 1, y: 2, w, h, coverage: vec![fill; usize::from(w) * usize::from(h)] }
    }

    fn blob() -> alloc::vec::Vec<u8> {
        let mut sizes = vec![
            BakedSize {
                px: 16,
                cell_w: 10,
                cell_h: 22,
                baseline: 16,
                // Deliberately unsorted: the writer sorts.
                glyphs: vec![glyph('b', 0, 3, 4, 7), glyph('a', BOLD, 2, 2, 9), glyph('a', 0, 2, 3, 5)],
            },
            BakedSize { px: 32, cell_w: 19, cell_h: 43, baseline: 32, glyphs: vec![glyph('€', ITALIC, 5, 6, 200)] },
        ];
        write(&mut sizes)
    }

    #[test]
    fn writer_and_reader_agree_on_magic() {
        assert_eq!(WRITER_MAGIC, MAGIC);
    }

    #[test]
    fn reads_faces_and_metrics() {
        let data = blob();
        let font = Font::new(&data).unwrap();
        let faces: alloc::vec::Vec<_> = font.faces().collect();
        assert_eq!(faces.len(), 2);
        assert_eq!((faces[0].px, faces[0].cell_w, faces[0].cell_h, faces[0].baseline), (16, 10, 22, 16));
        assert_eq!(faces[0].glyph_count(), 3);
        assert_eq!((faces[1].px, faces[1].cell_w), (32, 19));
    }

    #[test]
    fn finds_glyphs_by_char_and_style() {
        let data = blob();
        let face = Font::new(&data).unwrap().face(0).unwrap();
        let a = face.glyph('a', 0).unwrap();
        assert_eq!((a.x, a.y, a.w, a.h), (1, 2, 2, 3));
        assert_eq!(a.coverage, &[5; 6]);
        assert_eq!(face.glyph('a', BOLD).unwrap().coverage, &[9; 4]);
        // Missing styles fall back to regular; missing chars give None.
        assert_eq!(face.glyph('b', BOLD | ITALIC).unwrap().coverage, &[7; 12]);
        assert_eq!(face.glyph('z', 0), None);
        let euro = Font::new(&data).unwrap().face(1).unwrap().glyph('€', ITALIC).unwrap();
        assert_eq!(euro.coverage.len(), 30);
    }

    #[test]
    fn rejects_empty_and_truncated_blobs() {
        assert!(Font::new(&[]).is_none());
        assert!(Font::new(b"NWF1\x05\x00\x00\x00").is_none());
        let data = blob();
        assert!(Font::new(&data[..20]).is_none());
    }
}
