use crate::platform::{Platform, TextAttribute, Color, KeyEvent, FileInfo};
use std::io::Result;

pub enum ModalResult<T> {
    Selected(T),
    Cancelled,
}

pub struct FileModal<P: Platform> {
    platform: P,
    files: Vec<FileInfo>,
    selected_index: usize,
    width: usize,
    height: usize,
}

impl<P: Platform> FileModal<P> {
    pub fn new(platform: P, path: &str) -> Result<Self> {
        let (width, height) = platform.get_screen_size();
        let files = platform.list_files(path)?;
        
        Ok(Self {
            platform,
            files,
            selected_index: 0,
            width,
            height,
        })
    }

    pub fn show(&mut self) -> Result<ModalResult<String>> {
        loop {
            self.render()?;
            let key = self.platform.read_key()?;
            
            match key {
                KeyEvent::Enter => {
                    if let Some(file) = self.files.get(self.selected_index) {
                        if !file.is_directory {
                            return Ok(ModalResult::Selected(file.name.clone()));
                        }
                    }
                }
                KeyEvent::Escape => return Ok(ModalResult::Cancelled),
                KeyEvent::Up => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }
                KeyEvent::Down => {
                    if self.selected_index < self.files.len().saturating_sub(1) {
                        self.selected_index += 1;
                    }
                }
                KeyEvent::CtrlN => {
                    return Ok(ModalResult::Selected("new_file.md".to_string()));
                }
                _ => {}
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        self.platform.clear_screen()?;
        
        let modal_width = 60.min(self.width - 4);
        let modal_height = 15.min(self.height - 4);
        let start_x = (self.width - modal_width) / 2;
        let start_y = (self.height - modal_height) / 2;

        self.draw_box(start_x, start_y, modal_width, modal_height)?;
        
        let title = "Select File";
        let title_x = start_x + (modal_width - title.len()) / 2;
        for (i, ch) in title.chars().enumerate() {
            self.platform.draw_char(title_x + i, start_y, ch, TextAttribute {
                bright: true,
                background: Color::Default,
            })?;
        }

        let content_start_y = start_y + 2;
        let content_height = modal_height - 4;
        
        for (i, file) in self.files.iter().enumerate().take(content_height) {
            let y = content_start_y + i;
            let is_selected = i == self.selected_index;
            
            let prefix = if is_selected { "> " } else { "  " };
            let display_name = if file.is_directory {
                format!("{}/", file.name)
            } else {
                file.name.clone()
            };
            
            let line = format!("{}{}", prefix, display_name);
            let max_len = modal_width - 4;
            let truncated = if line.len() > max_len {
                format!("{}...", &line[..max_len - 3])
            } else {
                line
            };
            
            for (j, ch) in truncated.chars().enumerate() {
                self.platform.draw_char(start_x + 2 + j, y, ch, if is_selected {
                    TextAttribute {
                        bright: true,
                        background: Color::Grey,
                    }
                } else {
                    TextAttribute::default()
                })?;
            }
        }

        let help_y = start_y + modal_height - 2;
        let help_text = "[↑↓] Navigate  [Enter] Open  [Ctrl+N] New";
        let help_x = start_x + (modal_width - help_text.len()) / 2;
        for (i, ch) in help_text.chars().enumerate() {
            self.platform.draw_char(help_x + i, help_y, ch, TextAttribute {
                bright: false,
                background: Color::Default,
            })?;
        }
        
        Ok(())
    }

    fn draw_box(&mut self, x: usize, y: usize, width: usize, height: usize) -> Result<()> {
        for i in 0..width {
            self.platform.draw_char(x + i, y, '─', TextAttribute::default())?;
            self.platform.draw_char(x + i, y + height - 1, '─', TextAttribute::default())?;
        }
        
        for i in 0..height {
            self.platform.draw_char(x, y + i, '│', TextAttribute::default())?;
            self.platform.draw_char(x + width - 1, y + i, '│', TextAttribute::default())?;
        }
        
        self.platform.draw_char(x, y, '┌', TextAttribute::default())?;
        self.platform.draw_char(x + width - 1, y, '┐', TextAttribute::default())?;
        self.platform.draw_char(x, y + height - 1, '└', TextAttribute::default())?;
        self.platform.draw_char(x + width - 1, y + height - 1, '┘', TextAttribute::default())?;
        
        Ok(())
    }
}

pub struct KeyboardModal<P: Platform> {
    platform: P,
    layouts: Vec<(&'static str, &'static str)>,
    selected_index: usize,
    width: usize,
    height: usize,
}

impl<P: Platform> KeyboardModal<P> {
    pub fn new(platform: P) -> Self {
        let (width, height) = platform.get_screen_size();
        let layouts = vec![
            ("en-US", "English - United States"),
            ("de-CH", "German - Switzerland"),
            ("fr-FR", "French - France"),
            ("es-ES", "Spanish - Spain"),
        ];
        
        Self {
            platform,
            layouts,
            selected_index: 0,
            width,
            height,
        }
    }

    pub fn show(&mut self) -> Result<ModalResult<String>> {
        loop {
            self.render()?;
            let key = self.platform.read_key()?;
            
            match key {
                KeyEvent::Enter => {
                    if let Some((code, _)) = self.layouts.get(self.selected_index) {
                        return Ok(ModalResult::Selected(code.to_string()));
                    }
                }
                KeyEvent::Escape => return Ok(ModalResult::Cancelled),
                KeyEvent::Up => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }
                KeyEvent::Down => {
                    if self.selected_index < self.layouts.len().saturating_sub(1) {
                        self.selected_index += 1;
                    }
                }
                _ => {}
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        self.platform.clear_screen()?;
        
        let modal_width = 50.min(self.width - 4);
        let modal_height = 10.min(self.height - 4);
        let start_x = (self.width - modal_width) / 2;
        let start_y = (self.height - modal_height) / 2;

        self.draw_box(start_x, start_y, modal_width, modal_height)?;
        
        let title = "Select Keyboard Layout";
        let title_x = start_x + (modal_width - title.len()) / 2;
        for (i, ch) in title.chars().enumerate() {
            self.platform.draw_char(title_x + i, start_y, ch, TextAttribute {
                bright: true,
                background: Color::Default,
            })?;
        }

        let content_start_y = start_y + 2;
        
        for (i, (code, name)) in self.layouts.iter().enumerate() {
            let y = content_start_y + i;
            let is_selected = i == self.selected_index;
            
            let prefix = if is_selected { "> " } else { "  " };
            let line = format!("{}{} ({})", prefix, code, name);
            
            for (j, ch) in line.chars().enumerate() {
                if j < modal_width - 4 {
                    self.platform.draw_char(start_x + 2 + j, y, ch, if is_selected {
                        TextAttribute {
                            bright: true,
                            background: Color::Grey,
                        }
                    } else {
                        TextAttribute::default()
                    })?;
                }
            }
        }

        let help_y = start_y + modal_height - 2;
        let help_text = "[↑↓] Navigate  [Enter] Select";
        let help_x = start_x + (modal_width - help_text.len()) / 2;
        for (i, ch) in help_text.chars().enumerate() {
            self.platform.draw_char(help_x + i, help_y, ch, TextAttribute {
                bright: false,
                background: Color::Default,
            })?;
        }
        
        Ok(())
    }

    fn draw_box(&mut self, x: usize, y: usize, width: usize, height: usize) -> Result<()> {
        for i in 0..width {
            self.platform.draw_char(x + i, y, '─', TextAttribute::default())?;
            self.platform.draw_char(x + i, y + height - 1, '─', TextAttribute::default())?;
        }
        
        for i in 0..height {
            self.platform.draw_char(x, y + i, '│', TextAttribute::default())?;
            self.platform.draw_char(x + width - 1, y + i, '│', TextAttribute::default())?;
        }
        
        self.platform.draw_char(x, y, '┌', TextAttribute::default())?;
        self.platform.draw_char(x + width - 1, y, '┐', TextAttribute::default())?;
        self.platform.draw_char(x, y + height - 1, '└', TextAttribute::default())?;
        self.platform.draw_char(x + width - 1, y + height - 1, '┘', TextAttribute::default())?;
        
        Ok(())
    }
}