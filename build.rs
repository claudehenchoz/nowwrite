//! Bakes JetBrains Mono into `OUT_DIR/font.bin` for the UEFI graphics
//! backend (format: `src/font/writer.rs`). Other builds get an empty file,
//! so terminal builds stay fast.

extern crate alloc;

#[path = "src/font/writer.rs"]
mod writer;

use std::{env, fs, path::PathBuf};
use writer::{BakedGlyph, BakedSize};

/// Pixel sizes to bake; the backend picks the largest that fits 80x25.
const SIZES: [f32; 5] = [16.0, 20.0, 24.0, 28.0, 32.0];

/// Regular, bold, italic, bold italic: index = style bits (1 bold, 2 italic).
const STYLES: [&str; 4] = ["Regular", "Bold", "Italic", "BoldItalic"];

/// Coverage gamma: a' = 255 * (a/255)^GAMMA. Below 1 thickens strokes a
/// little, which keeps thin anti-aliased text legible on both dark and
/// light themes without a gamma-correct blend.
const GAMMA: f32 = 0.85;

/// Minimum cell height relative to the pixel size, so prose has room.
const MIN_LINE_HEIGHT: f32 = 1.35;

/// Characters beyond the Latin blocks: typography, arrows and maths.
const EXTRA: &str = "–—‘’“”„‚…•€‹›«»←↑→↓↵≠≤≥×·⋅′″";

fn chars() -> Vec<char> {
    let ranges = [0x20..=0x7E, 0xA0..=0xFF, 0x100..=0x17F];
    let mut out: Vec<char> = ranges.into_iter().flatten().filter_map(char::from_u32).collect();
    out.extend(EXTRA.chars());
    // Box drawing and block elements are drawn procedurally by the backend.
    out.retain(|&c| !('\u{2500}'..='\u{259F}').contains(&c));
    out.sort_unstable();
    out.dedup();
    out
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/font/writer.rs");
    println!("cargo:rerun-if-changed=assets/fonts");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("font.bin");

    if env::var_os("CARGO_FEATURE_UEFI").is_none() {
        fs::write(&out, []).unwrap();
        return;
    }

    let fonts: Vec<fontdue::Font> = STYLES
        .iter()
        .map(|style| {
            let path = format!("assets/fonts/JetBrainsMono-{style}.ttf");
            println!("cargo:rerun-if-changed={path}");
            let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).unwrap()
        })
        .collect();

    let gamma: Vec<u8> = (0..=255u32)
        .map(|a| (255.0 * (a as f32 / 255.0).powf(GAMMA)).round() as u8)
        .collect();

    let chars = chars();
    let mut sizes = Vec::new();
    for px in SIZES {
        let regular = &fonts[0];
        let line = regular.horizontal_line_metrics(px).unwrap();
        let cell_w = regular.metrics('M', px).advance_width.round() as i32;
        let natural = (line.ascent - line.descent + line.line_gap).round() as i32;
        let cell_h = natural.max((px * MIN_LINE_HEIGHT).round() as i32);
        let baseline = line.ascent.round() as i32 + (cell_h - natural) / 2;

        let mut glyphs = Vec::new();
        for (style, font) in fonts.iter().enumerate() {
            for &ch in &chars {
                if font.lookup_glyph_index(ch) == 0 {
                    continue; // not in the font: the backend draws a box
                }
                let (m, bitmap) = font.rasterize(ch, px);
                // Glyph box in cell coordinates, clipped to the cell.
                let left = m.xmin;
                let top = baseline - (m.ymin + m.height as i32);
                let (x0, y0) = (left.max(0), top.max(0));
                let x1 = (left + m.width as i32).min(cell_w);
                let y1 = (top + m.height as i32).min(cell_h);
                // Tighten to the rows/columns that have any ink.
                let ink = |x: i32, y: i32| bitmap[((y - top) * m.width as i32 + (x - left)) as usize] > 0;
                let rows: Vec<i32> = (y0..y1).filter(|&y| (x0..x1).any(|x| ink(x, y))).collect();
                let cols: Vec<i32> = (x0..x1).filter(|&x| (y0..y1).any(|y| ink(x, y))).collect();
                let (Some(&ty0), Some(&ty1), Some(&tx0), Some(&tx1)) =
                    (rows.first(), rows.last(), cols.first(), cols.last())
                else {
                    // Blank glyphs (space) still count as present.
                    glyphs.push(BakedGlyph { ch: ch as u32, style: style as u8, x: 0, y: 0, w: 0, h: 0, coverage: Vec::new() });
                    continue;
                };
                let mut coverage = Vec::new();
                for y in ty0..=ty1 {
                    for x in tx0..=tx1 {
                        coverage.push(gamma[bitmap[((y - top) * m.width as i32 + (x - left)) as usize] as usize]);
                    }
                }
                glyphs.push(BakedGlyph {
                    ch: ch as u32,
                    style: style as u8,
                    x: tx0 as u8,
                    y: ty0 as u8,
                    w: (tx1 - tx0 + 1) as u8,
                    h: (ty1 - ty0 + 1) as u8,
                    coverage,
                });
            }
        }
        sizes.push(BakedSize {
            px: px as u16,
            cell_w: cell_w as u16,
            cell_h: cell_h as u16,
            baseline: baseline as u16,
            glyphs,
        });
    }

    let blob = writer::write(&mut sizes);
    let summary: Vec<String> =
        sizes.iter().map(|s| format!("{}px={}x{}", s.px, s.cell_w, s.cell_h)).collect();
    println!(
        "cargo:warning=baked JetBrains Mono: {} glyphs/style, {} KiB ({})",
        chars.len(),
        blob.len() / 1024,
        summary.join(" ")
    );
    fs::write(&out, blob).unwrap();
}
