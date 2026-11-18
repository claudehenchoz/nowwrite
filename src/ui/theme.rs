//! Colour themes. Every colour on screen comes from the active theme, which
//! render code reads through [`current`] instead of threading it through
//! every function. The UEFI backends read it too, to resolve `Color::Reset`.

use core::sync::atomic::{AtomicUsize, Ordering};
use ratatui::style::Color;

/// A foreground/background pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    pub fg: Color,
    pub bg: Color,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Theme {
    /// Written to `.nowwrite/.theme.config`.
    pub id: &'static str,
    pub name: &'static str,
    /// Every editor cell is painted with these.
    pub background: Color,
    pub text: Color,
    pub heading: Color,
    pub bold: Color,
    pub italic: Color,
    /// The `#` and `*` markers; `None` uses the colour of their span.
    pub marker: Option<Color>,
    /// De-emphasized text such as hints.
    pub hint: Color,
    pub selection: Pair,
    /// The software block cursor.
    pub cursor: Pair,
    pub bar: Pair,
    /// The `F1 Help` hint in the status bar.
    pub bar_key: Color,
    pub dialog: Pair,
    pub dialog_border: Color,
    pub dialog_title: Color,
    /// Highlighted row in a dialog's list.
    pub dialog_selected: Pair,
    pub directory: Color,
    pub error: Color,
    /// Rounded modal corners (shown square where the font lacks them).
    pub rounded: bool,
}

const fn hex(rgb: u32) -> Color {
    Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

const fn pair(fg: u32, bg: u32) -> Pair {
    Pair { fg: hex(fg), bg: hex(bg) }
}

/// Today's look on the UEFI text console, in VGA palette colours.
const CLASSIC: Theme = Theme {
    id: "classic",
    name: "Classic",
    background: hex(0x000000),
    text: hex(0xAAAAAA),
    heading: hex(0xFFFFFF),
    bold: hex(0xFFFFFF),
    italic: hex(0xFFFFFF),
    marker: None,
    hint: hex(0x555555),
    selection: pair(0x000000, 0xAAAAAA),
    cursor: pair(0x000000, 0xAAAAAA),
    bar: pair(0x000000, 0xAAAAAA),
    bar_key: hex(0x000000),
    dialog: pair(0xAAAAAA, 0x000000),
    dialog_border: hex(0xAAAAAA),
    dialog_title: hex(0xFFFFFF),
    dialog_selected: pair(0x000000, 0xAAAAAA),
    directory: hex(0xAAAAAA),
    error: hex(0xFFFFFF),
    rounded: false,
};

/// After the Midnight Commander skin of the same name.
const NOCTURNE: Theme = Theme {
    id: "nocturne",
    name: "Nocturne",
    background: hex(0x14151f),
    text: hex(0xc9cce3),
    heading: hex(0xa9b1ff),
    bold: hex(0xf5c76e),
    italic: hex(0xc4a7f5),
    marker: Some(hex(0x4f5470)),
    hint: hex(0x737899),
    selection: pair(0xeef0ff, 0x363c63),
    cursor: pair(0x14151f, 0xa9b1ff),
    bar: pair(0x737899, 0x1b1d2b),
    bar_key: hex(0xa9b1ff),
    dialog: pair(0xc9cce3, 0x232638),
    dialog_border: hex(0xa9b1ff),
    dialog_title: hex(0xa9b1ff),
    dialog_selected: pair(0xeef0ff, 0x363c63),
    directory: hex(0x82c8ff),
    error: hex(0xf2778a),
    rounded: true,
};

const PAPER: Theme = Theme {
    id: "paper",
    name: "Paper",
    background: hex(0xf6f1e7),
    text: hex(0x3b3a36),
    heading: hex(0x1f4e79),
    bold: hex(0x8a4b08),
    italic: hex(0x6b3fa0),
    marker: Some(hex(0xb3ab9b)),
    hint: hex(0x8c8577),
    selection: pair(0x1c1b18, 0xd9cfb8),
    cursor: pair(0xf6f1e7, 0x3b3a36),
    bar: pair(0x5c584e, 0xe8e0cf),
    bar_key: hex(0x1f4e79),
    dialog: pair(0x3b3a36, 0xefe8da),
    dialog_border: hex(0x1f4e79),
    dialog_title: hex(0x1f4e79),
    dialog_selected: pair(0x1c1b18, 0xd9cfb8),
    directory: hex(0x1f4e79),
    error: hex(0xb3261e),
    rounded: true,
};

const AMBER: Theme = Theme {
    id: "amber",
    name: "Amber CRT",
    background: hex(0x0f0b05),
    text: hex(0xe8a33d),
    heading: hex(0xffcc66),
    bold: hex(0xffcc66),
    italic: hex(0xf0b860),
    marker: Some(hex(0x7a5520)),
    hint: hex(0x7a5520),
    selection: pair(0xffd98a, 0x4a3210),
    cursor: pair(0x0f0b05, 0xe8a33d),
    bar: pair(0xc48a35, 0x241a0a),
    bar_key: hex(0xffcc66),
    dialog: pair(0xe8a33d, 0x17110a),
    dialog_border: hex(0xc48a35),
    dialog_title: hex(0xffcc66),
    dialog_selected: pair(0xffd98a, 0x4a3210),
    directory: hex(0xffcc66),
    error: hex(0xff6a3d),
    rounded: false,
};

const PHOSPHOR: Theme = Theme {
    id: "phosphor",
    name: "Green Phosphor",
    background: hex(0x050d07),
    text: hex(0x4fd26b),
    heading: hex(0x9dffb0),
    bold: hex(0x9dffb0),
    italic: hex(0x7ce895),
    marker: Some(hex(0x1f6b33)),
    hint: hex(0x1f6b33),
    selection: pair(0xb8ffc6, 0x12401f),
    cursor: pair(0x050d07, 0x4fd26b),
    bar: pair(0x3fa556, 0x0b2412),
    bar_key: hex(0x9dffb0),
    dialog: pair(0x4fd26b, 0x08160c),
    dialog_border: hex(0x3fa556),
    dialog_title: hex(0x9dffb0),
    dialog_selected: pair(0xb8ffc6, 0x12401f),
    directory: hex(0x9dffb0),
    error: hex(0xffb347),
    rounded: false,
};

pub static THEMES: &[Theme] = &[CLASSIC, NOCTURNE, PAPER, AMBER, PHOSPHOR];

/// Index into `THEMES` of the theme used without a config file (Nocturne).
pub const DEFAULT: usize = 1;

static CURRENT: AtomicUsize = AtomicUsize::new(DEFAULT);

/// The active theme.
pub fn current() -> &'static Theme {
    &THEMES[current_index()]
}

pub fn current_index() -> usize {
    CURRENT.load(Ordering::Relaxed).min(THEMES.len() - 1)
}

/// Make `THEMES[index]` the active theme (out-of-range indexes are ignored).
pub fn set_current(index: usize) {
    if index < THEMES.len() {
        CURRENT.store(index, Ordering::Relaxed);
    }
}

pub fn index_of(id: &str) -> Option<usize> {
    THEMES.iter().position(|t| t.id == id)
}

/// Serializes tests that change the global theme.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 8-bit RGB triple.
pub type Rgb = (u8, u8, u8);

/// The 16 colours of the VGA / UEFI text console, in UEFI attribute order.
pub const VGA: [Rgb; 16] = [
    (0x00, 0x00, 0x00), // Black
    (0x00, 0x00, 0xAA), // Blue
    (0x00, 0xAA, 0x00), // Green
    (0x00, 0xAA, 0xAA), // Cyan
    (0xAA, 0x00, 0x00), // Red
    (0xAA, 0x00, 0xAA), // Magenta
    (0xAA, 0x55, 0x00), // Brown
    (0xAA, 0xAA, 0xAA), // LightGray
    (0x55, 0x55, 0x55), // DarkGray
    (0x55, 0x55, 0xFF), // LightBlue
    (0x55, 0xFF, 0x55), // LightGreen
    (0x55, 0xFF, 0xFF), // LightCyan
    (0xFF, 0x55, 0x55), // LightRed
    (0xFF, 0x55, 0xFF), // LightMagenta
    (0xFF, 0xFF, 0x55), // Yellow
    (0xFF, 0xFF, 0xFF), // White
];
pub const VGA_BLACK: u8 = 0;
pub const VGA_LIGHT_GRAY: u8 = 7;
pub const VGA_WHITE: u8 = 15;

/// RGB value of a ratatui colour; `Reset` becomes `default`. Named colours
/// use the VGA palette.
pub fn to_rgb(color: Color, default: Color) -> Rgb {
    let vga = |i: usize| VGA[i];
    match color {
        Color::Reset => match default {
            Color::Reset => VGA[0],
            other => to_rgb(other, Color::Reset),
        },
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => vga(0),
        Color::Red => vga(4),
        Color::Green => vga(2),
        Color::Yellow => vga(6),
        Color::Blue => vga(1),
        Color::Magenta => vga(5),
        Color::Cyan => vga(3),
        Color::Gray => vga(7),
        Color::DarkGray => vga(8),
        Color::LightRed => vga(12),
        Color::LightGreen => vga(10),
        Color::LightYellow => vga(14),
        Color::LightBlue => vga(9),
        Color::LightMagenta => vga(13),
        Color::LightCyan => vga(11),
        Color::White => vga(15),
        Color::Indexed(i) if i < 16 => {
            // ANSI order (black, red, green, yellow, blue, magenta, cyan, white).
            const ANSI_TO_VGA: [usize; 8] = [0, 4, 2, 6, 1, 5, 3, 7];
            vga(ANSI_TO_VGA[usize::from(i % 8)] + if i >= 8 { 8 } else { 0 })
        }
        Color::Indexed(_) => VGA[7],
    }
}

/// Perceptually weighted squared distance ("redmean").
fn distance(a: Rgb, b: Rgb) -> u32 {
    let rmean = (u32::from(a.0) + u32::from(b.0)) / 2;
    let d = |x: u8, y: u8| {
        let d = i32::from(x) - i32::from(y);
        (d * d) as u32
    };
    (((512 + rmean) * d(a.0, b.0)) >> 8) + 4 * d(a.1, b.1) + (((767 - rmean) * d(a.2, b.2)) >> 8)
}

/// Index of the VGA colour among the first `limit` closest to `rgb`.
pub fn nearest_vga(rgb: Rgb, limit: usize) -> u8 {
    (0..limit.min(16)).min_by_key(|&i| distance(rgb, VGA[i])).unwrap_or(0) as u8
}

/// Map a cell's colours to text-console attribute indexes `(fg, bg)`:
/// any of the 16 colours for text, the first 8 for backgrounds.
///
/// Highlighted backgrounds (selection, cursor, status bar, selected row)
/// that would vanish into the main background become black on light grey,
/// and text that would vanish into its background gets the most
/// contrasting colour.
pub fn console_colors(fg: Color, bg: Color, theme: &Theme) -> (u8, u8) {
    let main_bg = nearest_vga(to_rgb(theme.background, Color::Reset), 8);
    let bg_rgb = to_rgb(bg, theme.background);
    let bg_index = nearest_vga(bg_rgb, 8);
    let highlighted = bg != theme.background
        && [theme.selection.bg, theme.cursor.bg, theme.bar.bg, theme.dialog_selected.bg].contains(&bg);
    if highlighted && bg_index == main_bg {
        // On a light-grey screen (light themes) the inverse stands out.
        return if main_bg == VGA_LIGHT_GRAY { (VGA_WHITE, VGA_BLACK) } else { (VGA_BLACK, VGA_LIGHT_GRAY) };
    }
    let mut fg_index = nearest_vga(to_rgb(fg, theme.text), 16);
    if fg_index == bg_index {
        let bg_vga = VGA[usize::from(bg_index)];
        fg_index = (0..16).max_by_key(|&i| distance(bg_vga, VGA[i])).unwrap_or(15) as u8;
    }
    (fg_index, bg_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_default_is_nocturne() {
        for (i, theme) in THEMES.iter().enumerate() {
            assert_eq!(index_of(theme.id), Some(i));
        }
        assert_eq!(THEMES[DEFAULT].id, "nocturne");
        assert_eq!(index_of("nope"), None);
    }

    #[test]
    fn classic_maps_to_todays_console_colours() {
        let t = &THEMES[0];
        // Plain text, headings/bold/italic, hints.
        assert_eq!(console_colors(t.text, t.background, t), (7, 0));
        assert_eq!(console_colors(t.heading, t.background, t), (15, 0));
        assert_eq!(console_colors(t.hint, t.background, t), (8, 0));
        // Status bar, selection, cursor: black on light grey.
        assert_eq!(console_colors(t.bar.fg, t.bar.bg, t), (0, 7));
        assert_eq!(console_colors(t.selection.fg, t.selection.bg, t), (0, 7));
        assert_eq!(console_colors(t.cursor.fg, t.cursor.bg, t), (0, 7));
        // Reset resolves to the theme's text and background.
        assert_eq!(console_colors(Color::Reset, Color::Reset, t), (7, 0));
    }

    #[test]
    fn nearest_colour_and_background_limit() {
        assert_eq!(nearest_vga((0xFF, 0xFF, 0xFF), 16), 15);
        assert_eq!(nearest_vga((0xF0, 0x50, 0x50), 16), 12);
        // Backgrounds can't use the bright half: white falls back to light grey.
        assert_eq!(nearest_vga((0xFF, 0xFF, 0xFF), 8), 7);
        assert_eq!(nearest_vga((0x14, 0x15, 0x1f), 8), 0);
    }

    #[test]
    fn highlights_stay_visible_on_dark_themes() {
        for theme in THEMES {
            let main_bg = console_colors(theme.text, theme.background, theme).1;
            for p in [theme.selection, theme.cursor, theme.bar, theme.dialog_selected] {
                let (fg, bg) = console_colors(p.fg, p.bg, theme);
                assert_ne!(bg, main_bg, "{} highlight vanishes", theme.id);
                assert_ne!(fg, bg, "{} highlight text vanishes", theme.id);
            }
        }
        // Nocturne's selection (#363c63) becomes white on blue.
        let n = &THEMES[1];
        assert_eq!(console_colors(n.selection.fg, n.selection.bg, n), (15, 1));
    }

    #[test]
    #[ignore = "prints the console mapping of every theme"]
    fn print_console_mapping() {
        const NAMES: [&str; 16] = ["Black", "Blue", "Green", "Cyan", "Red", "Magenta", "Brown", "LightGray", "DarkGray", "LightBlue", "LightGreen", "LightCyan", "LightRed", "LightMagenta", "Yellow", "White"];
        for t in THEMES {
            let row = |name: &str, fg: Color, bg: Color| {
                let (f, b) = console_colors(fg, bg, t);
                std::println!("{:>10} {:>16}: {} on {}", t.id, name, NAMES[f as usize], NAMES[b as usize]);
            };
            row("text", t.text, t.background);
            row("heading", t.heading, t.background);
            row("bold", t.bold, t.background);
            row("italic", t.italic, t.background);
            row("marker", t.marker.unwrap_or(t.text), t.background);
            row("hint", t.hint, t.background);
            row("selection", t.selection.fg, t.selection.bg);
            row("cursor", t.cursor.fg, t.cursor.bg);
            row("bar", t.bar.fg, t.bar.bg);
            row("dialog", t.dialog.fg, t.dialog.bg);
            row("dialog_border", t.dialog_border, t.dialog.bg);
            row("dialog_selected", t.dialog_selected.fg, t.dialog_selected.bg);
            row("error", t.error, t.dialog.bg);
        }
    }

    #[test]
    fn unreadable_text_gets_contrasting_colour() {
        let t = &THEMES[0];
        // Black on black -> white.
        assert_eq!(console_colors(hex(0x000000), hex(0x000000), t), (15, 0));
        // Light grey on light grey -> black (the most distant colour).
        assert_eq!(console_colors(hex(0xAAAAAA), hex(0xAAAAAA), &THEMES[2]).0, 0);
    }

    #[test]
    fn named_and_indexed_colours_use_vga() {
        assert_eq!(to_rgb(Color::Gray, Color::Reset), VGA[7]);
        assert_eq!(to_rgb(Color::Indexed(9), Color::Reset), VGA[12]); // bright red
        assert_eq!(to_rgb(Color::Reset, hex(0x123456)), (0x12, 0x34, 0x56));
    }
}
