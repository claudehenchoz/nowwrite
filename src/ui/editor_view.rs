use crate::editor::buffer::TextBuffer;
use crate::editor::markdown::{highlight, SpanKind};
use alloc::vec::Vec;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The document area. Keeps the scroll position between frames.
#[derive(Default)]
pub struct EditorView {
    scroll_y: usize,
    scroll_x: usize,
    page_height: usize,
}

impl EditorView {
    /// Rows shown in the last frame, used for PageUp/PageDown.
    pub fn page_height(&self) -> usize {
        self.page_height.max(1)
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, buffer: &TextBuffer, show_cursor: bool) {
        let height = usize::from(area.height);
        let width = usize::from(area.width);
        self.page_height = height;
        let cursor = buffer.cursor();

        // Keep the cursor on screen.
        if cursor.line < self.scroll_y {
            self.scroll_y = cursor.line;
        } else if height > 0 && cursor.line >= self.scroll_y + height {
            self.scroll_y = cursor.line + 1 - height;
        }
        if cursor.column < self.scroll_x {
            self.scroll_x = cursor.column;
        } else if width > 0 && cursor.column >= self.scroll_x + width {
            self.scroll_x = cursor.column + 1 - width;
        }

        let lines: Vec<Line> = buffer
            .lines()
            .iter()
            .skip(self.scroll_y)
            .take(height)
            .map(|line| {
                let spans: Vec<Span> = highlight(line)
                    .into_iter()
                    .map(|(kind, text)| Span::styled(text, style_for(kind)))
                    .collect();
                Line::from(spans)
            })
            .collect();

        let scroll_x = u16::try_from(self.scroll_x).unwrap_or(u16::MAX);
        frame.render_widget(Paragraph::new(lines).scroll((0, scroll_x)), area);

        if show_cursor {
            let x = area.x + (cursor.column - self.scroll_x) as u16;
            let y = area.y + (cursor.line - self.scroll_y) as u16;
            frame.set_cursor_position(Position::new(x, y));
        }
    }
}

fn style_for(kind: SpanKind) -> Style {
    match kind {
        SpanKind::Plain => Style::new(),
        SpanKind::Heading | SpanKind::Bold => super::bright(),
        SpanKind::Italic => super::italic(),
    }
}
