//! Keyboard layouts for platforms whose firmware reports US-layout
//! characters (UEFI). Each layout maps the character a US keyboard would
//! produce for a physical key to what the user's keyboard has printed on it.

use crate::error::Result;
use crate::file::NOWWRITE_DIR;
use crate::platform::Platform;
use alloc::format;
use alloc::vec::Vec;

const CONFIG_PATH: &str = ".nowwrite/.keyboard.config";

/// What a key produces under a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Char(char),
    /// A dead key: combines with the next character (e.g. `^` + `e` = `ê`).
    Dead(char),
}

use Output::{Char as C, Dead as D};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardLayout {
    pub code: &'static str,
    pub name: &'static str,
    /// US character (with or without Shift) -> output.
    normal: &'static [(char, Output)],
    /// US character typed with AltGr held -> output.
    altgr: &'static [(char, Output)],
}

/// Swiss German (de-CH), mapped from the US characters of the same keys.
const DE_CH_NORMAL: &[(char, Output)] = &[
    // Number row.
    ('`', C('§')), ('~', C('°')),
    ('!', C('+')), ('@', C('"')), ('#', C('*')), ('$', C('ç')), ('%', C('%')),
    ('^', C('&')), ('&', C('/')), ('*', C('(')), ('(', C(')')), (')', C('=')),
    ('-', C('\'')), ('_', C('?')),
    ('=', D('^')), ('+', D('`')),
    // Top row.
    ('y', C('z')), ('Y', C('Z')),
    ('[', C('ü')), ('{', C('è')),
    (']', D('¨')), ('}', C('!')),
    // Home row.
    (';', C('ö')), (':', C('é')),
    ('\'', C('ä')), ('"', C('à')),
    ('\\', C('$')), ('|', C('£')),
    // Bottom row.
    ('z', C('y')), ('Z', C('Y')),
    ('<', C(';')), ('>', C(':')),
    ('/', C('-')), ('?', C('_')),
];

const DE_CH_ALTGR: &[(char, Output)] = &[
    ('1', C('¦')), ('2', C('@')), ('3', C('#')), ('4', C('°')), ('5', C('§')),
    ('6', C('¬')), ('7', C('|')), ('8', C('¢')),
    ('-', D('´')), ('=', D('~')),
    ('e', C('€')),
    ('[', C('[')), (']', C(']')),
    ('\'', C('{')), ('\\', C('}')),
    // The ISO key left of Y (`<` `>`, AltGr `\`) isn't in the firmware's US
    // keymap and produces nothing, so offer these on keys that have no
    // AltGr meaning on a Swiss keyboard: AltGr+, AltGr+. AltGr+-
    (',', C('<')), ('.', C('>')), ('/', C('\\')),
];

pub const LAYOUTS: &[KeyboardLayout] = &[
    KeyboardLayout {
        code: "en-US",
        name: "English - United States",
        normal: &[],
        altgr: &[],
    },
    KeyboardLayout {
        code: "de-CH",
        name: "German - Switzerland",
        normal: DE_CH_NORMAL,
        altgr: DE_CH_ALTGR,
    },
];

impl KeyboardLayout {
    pub fn by_code(code: &str) -> Option<KeyboardLayout> {
        LAYOUTS.iter().copied().find(|l| l.code == code)
    }

    /// Map a US-layout character to this layout. With AltGr, keys that have
    /// no AltGr meaning produce nothing.
    pub fn map(&self, ch: char, altgr: bool) -> Option<Output> {
        let table = if altgr { self.altgr } else { self.normal };
        match table.iter().find(|(from, _)| *from == ch) {
            Some((_, out)) => Some(*out),
            None if altgr && !self.altgr.is_empty() => None,
            None => Some(Output::Char(ch)),
        }
    }

}

/// Combine a dead-key accent with a base character.
fn compose(accent: char, base: char) -> Option<char> {
    const TABLE: &[(char, &str, &str)] = &[
        // accent, base chars, composed chars (same order)
        ('^', "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        ('`', "aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
        ('´', "aeiouyAEIOUY", "áéíóúýÁÉÍÓÚÝ"),
        ('¨', "aeiouyAEIOU", "äëïöüÿÄËÏÖÜ"),
        ('~', "anoANO", "ãñõÃÑÕ"),
    ];
    let (_, bases, composed) = TABLE.iter().find(|(a, _, _)| *a == accent)?;
    let index = bases.chars().position(|c| c == base)?;
    composed.chars().nth(index)
}

/// The selected layout (persisted to `.nowwrite/.keyboard.config`) plus
/// any pending dead key.
pub struct KeyboardConfig {
    current: KeyboardLayout,
    pending_dead: Option<char>,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self { current: LAYOUTS[0], pending_dead: None }
    }
}

impl KeyboardConfig {
    pub fn load<P: Platform>(platform: &mut P) -> Self {
        let mut config = Self::default();
        if let Ok(content) = platform.read_file(CONFIG_PATH) {
            for line in content.lines() {
                if let Some(("layout", value)) = line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                    if let Some(layout) = KeyboardLayout::by_code(value) {
                        config.current = layout;
                    }
                }
            }
        }
        config
    }

    pub fn set_layout<P: Platform>(&mut self, platform: &mut P, code: &str) -> Result<()> {
        if let Some(layout) = KeyboardLayout::by_code(code) {
            self.current = layout;
            self.pending_dead = None;
            if !platform.exists(NOWWRITE_DIR) {
                platform.create_directory(NOWWRITE_DIR)?;
            }
            platform.write_file(CONFIG_PATH, &format!("layout={}\n", layout.code))?;
        }
        Ok(())
    }

    pub fn current(&self) -> KeyboardLayout {
        self.current
    }

    /// Translate a typed US-layout character into the characters to insert
    /// (none while a dead key is pending, two if a dead key doesn't combine).
    pub fn translate(&mut self, ch: char, altgr: bool) -> Vec<char> {
        let mut out = Vec::new();
        match self.current.map(ch, altgr) {
            None => {}
            Some(Output::Dead(accent)) => match self.pending_dead.take() {
                // Pressing a dead key twice types the accent itself.
                Some(previous) => {
                    out.push(previous);
                    if previous != accent {
                        self.pending_dead = Some(accent);
                    }
                }
                None => self.pending_dead = Some(accent),
            },
            Some(Output::Char(c)) => match self.pending_dead.take() {
                Some(accent) if c == ' ' => out.push(accent),
                Some(accent) => match compose(accent, c) {
                    Some(composed) => out.push(composed),
                    None => {
                        out.push(accent);
                        out.push(c);
                    }
                },
                None => out.push(c),
            },
        }
        out
    }

    /// Forget a pending dead key (e.g. on Enter or cursor movement).
    pub fn cancel_dead_key(&mut self) {
        self.pending_dead = None;
    }

    pub fn layouts(&self) -> Vec<KeyboardLayout> {
        LAYOUTS.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn type_str(config: &mut KeyboardConfig, keys: &[(char, bool)]) -> String {
        keys.iter().flat_map(|&(c, altgr)| config.translate(c, altgr)).collect()
    }

    fn de_ch() -> KeyboardConfig {
        KeyboardConfig { current: KeyboardLayout::by_code("de-CH").unwrap(), pending_dead: None }
    }

    #[test]
    fn us_layout_is_identity() {
        let mut config = KeyboardConfig::default();
        assert_eq!(type_str(&mut config, &[('a', false), ('[', false), ('2', true)]), "a[2");
    }

    #[test]
    fn de_ch_letters_and_symbols() {
        let mut c = de_ch();
        assert_eq!(type_str(&mut c, &[('z', false), ('y', false), ('Z', false)]), "yzY");
        assert_eq!(type_str(&mut c, &[('[', false), (';', false), ('\'', false)]), "üöä");
        assert_eq!(type_str(&mut c, &[('{', false), (':', false), ('"', false)]), "èéà");
        assert_eq!(type_str(&mut c, &[('!', false), ('@', false), ('#', false), ('$', false)]), "+\"*ç");
        assert_eq!(type_str(&mut c, &[('&', false), ('*', false), ('(', false), (')', false)]), "/()=");
        assert_eq!(type_str(&mut c, &[('-', false), ('_', false), ('/', false), ('?', false)]), "'?-_");
        assert_eq!(type_str(&mut c, &[('<', false), ('>', false), ('}', false)]), ";:!");
    }

    #[test]
    fn de_ch_altgr() {
        let mut c = de_ch();
        assert_eq!(type_str(&mut c, &[('2', true), ('3', true), ('7', true), ('e', true)]), "@#|€");
        assert_eq!(type_str(&mut c, &[('[', true), (']', true), ('\'', true), ('\\', true)]), "[]{}");
        // Substitutes for the ISO key: AltGr + , . - (US , . /)
        assert_eq!(type_str(&mut c, &[(',', true), ('.', true), ('/', true)]), "<>\\");
        // AltGr on a key without an AltGr character types nothing.
        assert_eq!(type_str(&mut c, &[('q', true)]), "");
    }

    #[test]
    fn de_ch_dead_keys() {
        let mut c = de_ch();
        // ^ + e, ` + a, ¨ + o, ´ (AltGr) + e, ~ (AltGr) + n
        assert_eq!(type_str(&mut c, &[('=', false), ('e', false)]), "ê");
        assert_eq!(type_str(&mut c, &[('+', false), ('a', false)]), "à");
        assert_eq!(type_str(&mut c, &[(']', false), ('o', false)]), "ö");
        assert_eq!(type_str(&mut c, &[('-', true), ('e', false)]), "é");
        assert_eq!(type_str(&mut c, &[('=', true), ('n', false)]), "ñ");
        // Space gives the accent itself; a non-combining char gives both.
        assert_eq!(type_str(&mut c, &[('=', false), (' ', false)]), "^");
        assert_eq!(type_str(&mut c, &[('=', false), ('x', false)]), "^x");
        assert_eq!(type_str(&mut c, &[('=', false), ('=', false)]), "^");
    }
}
