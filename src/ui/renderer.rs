use crate::platform::{Platform, TextAttribute, Color};
use crate::editor::buffer::TextBuffer;
use crate::editor::markdown::{MarkdownParser, MarkdownToken};
use std::io::Result;

pub struct Renderer<P: Platform> {
    platform: P,
    width: usize,
    height: usize,
    scroll_offset: usize,
}

impl<P: Platform> Renderer<P> {
    pub fn new(mut platform: P) -> Result<Self> {
        let (width, height) = platform.get_screen_size();
        platform.clear_screen()?;
        
        Ok(Self {
            platform,
            width,
            height,
            scroll_offset: 0,
        })
    }

    pub fn render_buffer(&mut self, buffer: &TextBuffer) -> Result<()> {
        self.platform.clear_screen()?;
        
        let lines = buffer.lines();
        let cursor = buffer.cursor();
        let selection = buffer.selection();
        
        let content_height = self.height.saturating_sub(1);
        
        if cursor.line >= self.scroll_offset + content_height {
            self.scroll_offset = cursor.line.saturating_sub(content_height.saturating_sub(1));
        } else if cursor.line < self.scroll_offset {
            self.scroll_offset = cursor.line;
        }

        let parsed_lines = MarkdownParser::parse_lines(lines);
        
        for (line_idx, markdown_line) in parsed_lines.iter().enumerate() {
            if line_idx < self.scroll_offset {
                continue;
            }
            
            let screen_line = line_idx - self.scroll_offset;
            if screen_line >= content_height {
                break;
            }

            let mut col = 0;
            for token in &markdown_line.tokens {
                col += self.render_token(screen_line, col, token, line_idx, selection)?;
            }
        }

        self.render_status_bar(buffer)?;
        self.render_cursor(cursor)?;
        
        Ok(())
    }

    fn render_token(
        &mut self,
        line: usize,
        start_col: usize,
        token: &MarkdownToken,
        _line_idx: usize,
        _selection: Option<crate::editor::cursor::Selection>,
    ) -> Result<usize> {
        let text = match token {
            MarkdownToken::Heading { text, level: _ } => text,
            MarkdownToken::Bold(text) => text,
            MarkdownToken::Italic(text) => text,
            MarkdownToken::PlainText(text) => text,
        };

        let attr = match token {
            MarkdownToken::Heading { level: _, text: _ } => TextAttribute {
                bright: true,
                background: Color::Default,
            },
            MarkdownToken::Bold(_) => TextAttribute {
                bright: true,
                background: Color::Default,
            },
            MarkdownToken::Italic(_) => TextAttribute {
                bright: false,
                background: Color::Grey,
            },
            MarkdownToken::PlainText(_) => TextAttribute::default(),
        };

        for (char_idx, ch) in text.chars().enumerate() {
            let col = start_col + char_idx;
            if col < self.width {
                self.platform.draw_char(col, line, ch, attr)?;
            }
        }

        Ok(text.chars().count())
    }

    fn render_status_bar(&mut self, buffer: &TextBuffer) -> Result<()> {
        let status_line = self.height - 1;
        
        let filename = buffer.file_path()
            .map(|p| p.as_str())
            .unwrap_or("untitled.md");
        
        let word_count = self.count_words(buffer);
        let char_count = buffer.to_string().len();
        let modified = if buffer.is_modified() { "●" } else { "" };
        
        let status = format!(" {} | {} words | {} chars | {} ", 
                           filename, word_count, char_count, modified);
        
        for (i, ch) in status.chars().enumerate() {
            if i < self.width {
                self.platform.draw_char(i, status_line, ch, TextAttribute {
                    bright: false,
                    background: Color::Grey,
                })?;
            }
        }
        
        for i in status.chars().count()..self.width {
            self.platform.draw_char(i, status_line, ' ', TextAttribute {
                bright: false,
                background: Color::Grey,
            })?;
        }
        
        Ok(())
    }

    fn render_cursor(&mut self, cursor: crate::editor::cursor::Cursor) -> Result<()> {
        if cursor.line >= self.scroll_offset && 
           cursor.line < self.scroll_offset + self.height.saturating_sub(1) {
            let screen_line = cursor.line - self.scroll_offset;
            if cursor.column < self.width {
                self.platform.draw_char(cursor.column, screen_line, '█', TextAttribute::default())?;
            }
        }
        Ok(())
    }

    fn count_words(&self, buffer: &TextBuffer) -> usize {
        buffer.to_string()
            .split_whitespace()
            .count()
    }

    pub fn get_platform_mut(&mut self) -> &mut P {
        &mut self.platform
    }
}