use crate::error::Result;
use crate::file::NOWWRITE_DIR;
use crate::platform::Platform;
use alloc::format;
use alloc::vec::Vec;

const CONFIG_PATH: &str = ".nowwrite/.keyboard.config";

/// A keyboard layout expressed as remappings from US-layout characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardLayout {
    pub code: &'static str,
    pub name: &'static str,
    mappings: &'static [(char, char)],
}

pub const LAYOUTS: &[KeyboardLayout] = &[
    KeyboardLayout {
        code: "en-US",
        name: "English - United States",
        mappings: &[],
    },
    KeyboardLayout {
        code: "de-CH",
        name: "German - Switzerland",
        mappings: &[('z', 'y'), ('y', 'z'), ('[', 'ü'), (';', 'ö'), ('\'', 'ä')],
    },
    KeyboardLayout {
        code: "fr-FR",
        name: "French - France",
        mappings: &[('q', 'a'), ('w', 'z'), ('a', 'q'), ('z', 'w'), ('[', '^'), (';', 'm')],
    },
    KeyboardLayout {
        code: "es-ES",
        name: "Spanish - Spain",
        mappings: &[('[', '´'), (';', 'ñ')],
    },
];

impl KeyboardLayout {
    pub fn by_code(code: &str) -> Option<KeyboardLayout> {
        LAYOUTS.iter().copied().find(|l| l.code == code)
    }

    pub fn map_char(&self, ch: char) -> char {
        self.mappings
            .iter()
            .find(|(from, _)| *from == ch)
            .map_or(ch, |(_, to)| *to)
    }
}

/// The selected layout, persisted to `.nowwrite/.keyboard.config`.
pub struct KeyboardConfig {
    current: KeyboardLayout,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self { current: LAYOUTS[0] }
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

    pub fn layouts(&self) -> Vec<KeyboardLayout> {
        LAYOUTS.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_us_layout() {
        let layout = KeyboardLayout::by_code("en-US").unwrap();
        assert_eq!(layout.map_char('a'), 'a');
        assert_eq!(layout.map_char('z'), 'z');
    }

    #[test]
    fn test_german_layout() {
        let layout = KeyboardLayout::by_code("de-CH").unwrap();
        assert_eq!(layout.map_char('z'), 'y');
        assert_eq!(layout.map_char('y'), 'z');
        assert_eq!(layout.map_char('a'), 'a'); // unmapped char
    }

    #[test]
    fn test_layout_lookup() {
        assert!(LAYOUTS.iter().any(|l| l.code == "en-US"));
        assert!(LAYOUTS.iter().any(|l| l.code == "de-CH"));
    }
}
