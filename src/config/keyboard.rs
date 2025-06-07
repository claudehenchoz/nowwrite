use crate::platform::Platform;
use std::io::Result;
use std::path::Path;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    pub code: String,
    pub name: String,
    pub mappings: HashMap<char, char>,
}

impl KeyboardLayout {
    pub fn us_english() -> Self {
        Self {
            code: "en-US".to_string(),
            name: "English - United States".to_string(),
            mappings: HashMap::new(), // No remapping needed for base layout
        }
    }

    pub fn german_swiss() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert('z', 'y');
        mappings.insert('y', 'z');
        mappings.insert('[', 'ü');
        mappings.insert(';', 'ö');
        mappings.insert('\'', 'ä');
        
        Self {
            code: "de-CH".to_string(),
            name: "German - Switzerland".to_string(),
            mappings,
        }
    }

    pub fn french() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert('q', 'a');
        mappings.insert('w', 'z');
        mappings.insert('a', 'q');
        mappings.insert('z', 'w');
        mappings.insert('[', '^');
        mappings.insert(';', 'm');
        
        Self {
            code: "fr-FR".to_string(),
            name: "French - France".to_string(),
            mappings,
        }
    }

    pub fn spanish() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert('[', '´');
        mappings.insert(';', 'ñ');
        
        Self {
            code: "es-ES".to_string(),
            name: "Spanish - Spain".to_string(),
            mappings,
        }
    }

    pub fn map_char(&self, ch: char) -> char {
        self.mappings.get(&ch).copied().unwrap_or(ch)
    }

    pub fn get_available_layouts() -> Vec<KeyboardLayout> {
        vec![
            Self::us_english(),
            Self::german_swiss(),
            Self::french(),
            Self::spanish(),
        ]
    }
}

pub struct KeyboardConfig<P: Platform> {
    platform: P,
    current_layout: KeyboardLayout,
    config_path: String,
}

impl<P: Platform> KeyboardConfig<P> {
    pub fn new(platform: P) -> Self {
        Self {
            platform,
            current_layout: KeyboardLayout::us_english(),
            config_path: ".nowwrite/.keyboard.config".to_string(),
        }
    }

    pub fn load(&mut self) -> Result<()> {
        if Path::new(&self.config_path).exists() {
            let content = self.platform.read_file(&self.config_path)?;
            
            for line in content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    match key.trim() {
                        "layout" => {
                            if let Some(layout) = self.find_layout_by_code(value.trim()) {
                                self.current_layout = layout;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        self.ensure_config_directory()?;
        
        let config_content = format!("layout={}\n", self.current_layout.code);
        self.platform.write_file(&self.config_path, &config_content)?;
        
        Ok(())
    }

    pub fn set_layout(&mut self, layout_code: &str) -> Result<bool> {
        if let Some(layout) = self.find_layout_by_code(layout_code) {
            self.current_layout = layout;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn current_layout(&self) -> &KeyboardLayout {
        &self.current_layout
    }

    pub fn map_input(&self, ch: char) -> char {
        self.current_layout.map_char(ch)
    }

    pub fn get_available_layouts(&self) -> Vec<KeyboardLayout> {
        KeyboardLayout::get_available_layouts()
    }

    fn find_layout_by_code(&self, code: &str) -> Option<KeyboardLayout> {
        KeyboardLayout::get_available_layouts()
            .into_iter()
            .find(|layout| layout.code == code)
    }

    fn ensure_config_directory(&mut self) -> Result<()> {
        let config_dir = ".nowwrite";
        if !Path::new(config_dir).exists() {
            self.platform.create_directory(config_dir)?;
        }
        Ok(())
    }

    pub fn get_layout_display_info(&self) -> String {
        format!("{} ({})", self.current_layout.name, self.current_layout.code)
    }

    pub fn reset_to_default(&mut self) -> Result<()> {
        self.current_layout = KeyboardLayout::us_english();
        self.save()
    }

    pub fn get_platform_mut(&mut self) -> &mut P {
        &mut self.platform
    }

    pub fn is_special_char(&self, ch: char) -> bool {
        self.current_layout.mappings.contains_key(&ch)
    }

    pub fn get_mapping_info(&self) -> Vec<(char, char)> {
        self.current_layout.mappings.iter().map(|(&k, &v)| (k, v)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_us_layout() {
        let layout = KeyboardLayout::us_english();
        assert_eq!(layout.map_char('a'), 'a');
        assert_eq!(layout.map_char('z'), 'z');
    }

    #[test]
    fn test_german_layout() {
        let layout = KeyboardLayout::german_swiss();
        assert_eq!(layout.map_char('z'), 'y');
        assert_eq!(layout.map_char('y'), 'z');
        assert_eq!(layout.map_char('a'), 'a'); // unmapped char
    }

    #[test]
    fn test_layout_lookup() {
        let layouts = KeyboardLayout::get_available_layouts();
        assert!(layouts.iter().any(|l| l.code == "en-US"));
        assert!(layouts.iter().any(|l| l.code == "de-CH"));
    }
}