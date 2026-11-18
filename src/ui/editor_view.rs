//! The document area: soft-wrapped lines with markdown highlighting,
//! selection, and screen-row based cursor movement.

use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::{Cursor, Selection};
use crate::editor::markdown::{highlight, SpanKind};
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Width used before the first frame is drawn.
const DEFAULT_WIDTH: usize = 80;

/// One screen row: chars `start..end` of buffer line `line`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// Split a line into rows of at most `width` chars, breaking after a space
/// where possible. The last row is always shorter than `width`, so a cursor
/// at the end of the line still fits on it.
pub fn wrap_line(chars: &[char], width: usize) -> Vec<(usize, usize)> {
    let width = width.max(2);
    let mut rows = Vec::new();
    let mut pos = 0;
    while chars.len() - pos >= width {
        let window = &chars[pos..pos + width];
        let brk = window
            .iter()
            .rposition(|&c| c == ' ')
            .map_or(pos + width, |i| pos + i + 1);
        rows.push((pos, brk));
        pos = brk;
    }
    rows.push((pos, chars.len()));
    rows
}

/// Screen rows for the whole document.
pub fn layout(lines: &[String], width: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    for (line, text) in lines.iter().enumerate() {
        let chars: Vec<char> = text.chars().collect();
        rows.extend(wrap_line(&chars, width).into_iter().map(|(start, end)| Row { line, start, end }));
    }
    rows
}

/// Index of the row holding `cursor`, and the cursor's x within that row.
pub fn cursor_row(rows: &[Row], cursor: Cursor) -> (usize, usize) {
    let first = rows.partition_point(|r| r.line < cursor.line);
    let mut index = first;
    while index + 1 < rows.len() && rows[index + 1].line == cursor.line && cursor.column >= rows[index].end {
        index += 1;
    }
    let row = rows.get(index).copied().unwrap_or(Row { line: cursor.line, start: 0, end: 0 });
    (index, cursor.column.saturating_sub(row.start))
}

fn is_last_row_of_line(rows: &[Row], index: usize) -> bool {
    rows.get(index + 1).is_none_or(|next| next.line != rows[index].line)
}

/// Furthest cursor x on a row: past the end on a line's last row, on the
/// last char (usually the break space) otherwise.
fn max_x(rows: &[Row], index: usize) -> usize {
    let row = rows[index];
    let len = row.end - row.start;
    if is_last_row_of_line(rows, index) { len } else { len.saturating_sub(1) }
}

#[derive(Default)]
pub struct EditorView {
    scroll: usize,
    width: usize,
    height: usize,
}

impl EditorView {
    fn wrap_width(&self) -> usize {
        if self.width == 0 { DEFAULT_WIDTH } else { self.width }
    }

    /// Rows shown in the last frame, used for PageUp/PageDown.
    pub fn page_height(&self) -> usize {
        self.height.max(1)
    }

    /// The cursor's x position within its screen row.
    pub fn cursor_x(&self, buffer: &TextBuffer) -> usize {
        cursor_row(&layout(buffer.lines(), self.wrap_width()), buffer.cursor()).1
    }

    /// The position `delta` screen rows above (negative) or below the cursor,
    /// as close to column `goal_x` as the target row allows.
    pub fn vertical(&self, buffer: &TextBuffer, delta: isize, goal_x: usize) -> Cursor {
        let rows = layout(buffer.lines(), self.wrap_width());
        let (index, _) = cursor_row(&rows, buffer.cursor());
        let target = index.saturating_add_signed(delta).min(rows.len() - 1);
        let row = rows[target];
        Cursor::new_at(row.line, row.start + goal_x.min(max_x(&rows, target)))
    }

    /// Start (`end == false`) or end of the cursor's screen row.
    pub fn row_edge(&self, buffer: &TextBuffer, end: bool) -> Cursor {
        let rows = layout(buffer.lines(), self.wrap_width());
        let (index, _) = cursor_row(&rows, buffer.cursor());
        let row = rows[index];
        let x = if end { max_x(&rows, index) } else { 0 };
        Cursor::new_at(row.line, row.start + x)
    }

    /// Draw the document; returns the cursor's screen position.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, buffer: &TextBuffer) -> Option<Position> {
        self.width = usize::from(area.width);
        self.height = usize::from(area.height);
        if area.is_empty() {
            return None;
        }
        let rows = layout(buffer.lines(), self.wrap_width());
        let (cursor_index, cursor_x) = cursor_row(&rows, buffer.cursor());

        // Keep the cursor row on screen.
        if cursor_index < self.scroll {
            self.scroll = cursor_index;
        } else if cursor_index >= self.scroll + self.height {
            self.scroll = cursor_index + 1 - self.height;
        }

        let selection = buffer.selection().filter(|s| !s.is_empty()).map(|s| s.normalize());
        let mut cached: Option<(usize, Vec<char>, Vec<Style>)> = None;
        let mut lines = Vec::new();
        for row in rows.iter().skip(self.scroll).take(self.height) {
            if cached.as_ref().is_none_or(|(line, _, _)| *line != row.line) {
                let text = &buffer.lines()[row.line];
                cached = Some((row.line, text.chars().collect(), char_styles(text, row.line, selection)));
            }
            let (_, chars, styles) = cached.as_ref().unwrap();
            lines.push(styled_line(&chars[row.start..row.end], &styles[row.start..row.end]));
        }
        frame.render_widget(Paragraph::new(lines), area);

        let x = area.x + cursor_x as u16;
        let y = area.y + (cursor_index - self.scroll) as u16;
        Some(Position::new(x.min(area.right() - 1), y))
    }
}

/// Style of every char of a line: markdown highlighting, then selection.
fn char_styles(text: &str, line: usize, selection: Option<Selection>) -> Vec<Style> {
    let mut styles = Vec::new();
    for (kind, span) in highlight(text) {
        styles.extend(core::iter::repeat_n(style_for(kind), span.chars().count()));
    }
    if let Some(Selection { start, end }) = selection {
        if (start.line..=end.line).contains(&line) {
            let from = if line == start.line { start.column } else { 0 };
            let to = if line == end.line { end.column } else { styles.len() };
            for style in styles.iter_mut().take(to).skip(from) {
                *style = super::selected();
            }
        }
    }
    styles
}

/// Group runs of equally styled chars into spans.
fn styled_line(chars: &[char], styles: &[Style]) -> Line<'static> {
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::new();
    for (&ch, &style) in chars.iter().zip(styles) {
        if style != run_style && !run.is_empty() {
            spans.push(Span::styled(core::mem::take(&mut run), run_style));
        }
        run_style = style;
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    Line::from(spans)
}

fn style_for(kind: SpanKind) -> Style {
    match kind {
        SpanKind::Plain => Style::new(),
        SpanKind::Heading | SpanKind::Bold => super::bright(),
        SpanKind::Italic => super::italic(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn wraps_at_spaces_and_hard_breaks_long_words() {
        assert_eq!(wrap_line(&chars("hello world"), 8), vec![(0, 6), (6, 11)]);
        assert_eq!(wrap_line(&chars("abcdefghij"), 4), vec![(0, 4), (4, 8), (8, 10)]);
        assert_eq!(wrap_line(&chars(""), 4), vec![(0, 0)]);
        // A line exactly as wide as the screen gets an empty last row for the cursor.
        assert_eq!(wrap_line(&chars("abcd"), 4), vec![(0, 4), (4, 4)]);
    }

    #[test]
    fn cursor_maps_to_rows() {
        let lines = vec!["hello world".to_string(), "x".to_string()];
        let rows = layout(&lines, 8);
        assert_eq!(rows.len(), 3);
        assert_eq!(cursor_row(&rows, Cursor::new_at(0, 3)), (0, 3));
        // The break position belongs to the next row.
        assert_eq!(cursor_row(&rows, Cursor::new_at(0, 6)), (1, 0));
        assert_eq!(cursor_row(&rows, Cursor::new_at(0, 11)), (1, 5));
        assert_eq!(cursor_row(&rows, Cursor::new_at(1, 1)), (2, 1));
    }

    #[test]
    fn vertical_movement_follows_rows() {
        let mut buffer = TextBuffer::from_string("hello world\nx");
        let view = EditorView { scroll: 0, width: 8, height: 10 };
        buffer.move_cursor(Cursor::new_at(0, 2));
        assert_eq!(view.vertical(&buffer, 1, 2), Cursor::new_at(0, 8));
        buffer.move_cursor(Cursor::new_at(0, 8));
        assert_eq!(view.vertical(&buffer, 1, 2), Cursor::new_at(1, 1));
        assert_eq!(view.vertical(&buffer, -1, 2), Cursor::new_at(0, 2));
        // End of a wrapped (non-last) row stops before the break.
        buffer.move_cursor(Cursor::new_at(0, 1));
        assert_eq!(view.row_edge(&buffer, true), Cursor::new_at(0, 5));
    }
}
