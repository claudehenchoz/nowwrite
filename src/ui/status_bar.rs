use crate::platform::{Platform, TextAttribute, Color};
use crate::editor::buffer::TextBuffer;
use std::io::Result;

pub struct StatusBar;

impl StatusBar {
    pub fn render<P: Platform>(
        platform: &mut P,
        buffer: &TextBuffer,
        width: usize,
        y: usize,
    ) -> Result<()> {
        let filename = buffer.file_path()
            .map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| "untitled.md".to_string());
        
        let word_count = Self::count_words(buffer);
        let char_count = buffer.to_string().len();
        let modified = if buffer.is_modified() { "●" } else { "" };
        
        let cursor = buffer.cursor();
        let position_info = format!("{}:{}", cursor.line + 1, cursor.column + 1);
        
        let left_info = format!(" {} | {} words | {} chars {}", 
                              filename, word_count, char_count, modified);
        
        let right_info = format!("{} ", position_info);
        
        let available_width = width.saturating_sub(left_info.len()).saturating_sub(right_info.len());
        
        for (i, ch) in left_info.chars().enumerate() {
            if i < width {
                platform.draw_char(i, y, ch, TextAttribute {
                    bright: false,
                    background: Color::Grey,
                })?;
            }
        }
        
        for i in left_info.len()..(left_info.len() + available_width) {
            if i < width {
                platform.draw_char(i, y, ' ', TextAttribute {
                    bright: false,
                    background: Color::Grey,
                })?;
            }
        }
        
        let right_start = width.saturating_sub(right_info.len());
        for (i, ch) in right_info.chars().enumerate() {
            let pos = right_start + i;
            if pos < width {
                platform.draw_char(pos, y, ch, TextAttribute {
                    bright: false,
                    background: Color::Grey,
                })?;
            }
        }
        
        Ok(())
    }

    fn count_words(buffer: &TextBuffer) -> usize {
        buffer.to_string()
            .split_whitespace()
            .filter(|word| !word.is_empty())
            .count()
    }

    pub fn render_help<P: Platform>(
        platform: &mut P,
        width: usize,
        y: usize,
        help_text: &str,
    ) -> Result<()> {
        let centered_x = if help_text.len() < width {
            (width - help_text.len()) / 2
        } else {
            0
        };
        
        for i in 0..width {
            platform.draw_char(i, y, ' ', TextAttribute {
                bright: false,
                background: Color::Grey,
            })?;
        }
        
        for (i, ch) in help_text.chars().enumerate() {
            let pos = centered_x + i;
            if pos < width {
                platform.draw_char(pos, y, ch, TextAttribute {
                    bright: true,
                    background: Color::Grey,
                })?;
            }
        }
        
        Ok(())
    }
}