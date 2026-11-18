//! The chosen theme, persisted to `.nowwrite/.theme.config`.

use crate::error::Result;
use crate::file::NOWWRITE_DIR;
use crate::platform::Platform;
use crate::ui::theme::{self, Theme, DEFAULT};
use alloc::format;

const CONFIG_PATH: &str = ".nowwrite/.theme.config";

/// Activate the saved theme, or the default without a config file or with
/// an unknown theme id.
pub fn load<P: Platform>(platform: &mut P) {
    let mut index = DEFAULT;
    if let Ok(content) = platform.read_file(CONFIG_PATH) {
        for line in content.lines() {
            if let Some(("theme", value)) = line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                index = theme::index_of(value).unwrap_or(DEFAULT);
            }
        }
    }
    theme::set_current(index);
}

pub fn save<P: Platform>(platform: &mut P, theme: &Theme) -> Result<()> {
    if !platform.exists(NOWWRITE_DIR) {
        platform.create_directory(NOWWRITE_DIR)?;
    }
    platform.write_file(CONFIG_PATH, &format!("theme={}\n", theme.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::mock::MockPlatform;
    use crate::ui::theme::{current, TEST_LOCK, THEMES};

    #[test]
    fn round_trip_and_fallbacks() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut platform = MockPlatform::default();

        // No config file: the default theme.
        theme::set_current(0);
        load(&mut platform);
        assert_eq!(current().id, "nocturne");

        save(&mut platform, &THEMES[3]).unwrap();
        assert!(platform.exists(".nowwrite"));
        assert_eq!(platform.read_file(CONFIG_PATH).unwrap(), "theme=amber\n");
        theme::set_current(0);
        load(&mut platform);
        assert_eq!(current().id, "amber");

        // Unknown ids fall back to the default.
        platform.write_file(CONFIG_PATH, "theme=neon\n").unwrap();
        load(&mut platform);
        assert_eq!(current().id, "nocturne");
    }
}
