//! The document area: soft-wrapped lines with markdown highlighting,
//! selection, and screen-row based cursor movement.

use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::{Cursor, Selection};
use crate::editor::markdown::{highlight, SpanKind};
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::layout::{Position, Rect};
use super::theme;
use ratatui::style::{Modifier, Style};
use ratatui::Frame;

/// Width used before the first frame is drawn.
const DEFAULT_WIDTH: usize = 80;

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

/// Rows (`start..end` char ranges) of one buffer line.
pub fn line_rows(text: &str, width: usize) -> Vec<(usize, usize)> {
    // Most lines fit on one row; skip collecting their chars.
    if text.len() < width.max(2) {
        return alloc::vec![(0, text.chars().count())];
    }
    let chars: Vec<char> = text.chars().collect();
    wrap_line(&chars, width)
}

/// Index of the row of a line holding char `column`. A break position
/// belongs to the next row.
pub fn sub_row(rows: &[(usize, usize)], column: usize) -> usize {
    let mut i = 0;
    while i + 1 < rows.len() && column >= rows[i].1 {
        i += 1;
    }
    i
}

/// Furthest cursor x on row `i`: past the end on a line's last row, on the
/// last char (usually the break space) otherwise.
fn max_x(rows: &[(usize, usize)], i: usize) -> usize {
    let (start, end) = rows[i];
    if i + 1 == rows.len() { end - start } else { (end - start).saturating_sub(1) }
}

/// A screen row, addressed as row `sub` of buffer line `line`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowPos {
    pub line: usize,
    pub sub: usize,
}

/// Walks screen rows from a position, wrapping each line only once.
/// Yields the position and the row's char range.
pub struct Rows<'a> {
    lines: &'a [String],
    width: usize,
    pos: RowPos,
    current: Vec<(usize, usize)>,
    done: bool,
}

impl<'a> Rows<'a> {
    pub fn new(lines: &'a [String], width: usize, pos: RowPos) -> Self {
        let current = line_rows(&lines[pos.line], width);
        Self { lines, width, pos, current, done: false }
    }
}

impl Iterator for Rows<'_> {
    type Item = (RowPos, (usize, usize));

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let item = (self.pos, self.current[self.pos.sub]);
        if self.pos.sub + 1 < self.current.len() {
            self.pos.sub += 1;
        } else if self.pos.line + 1 < self.lines.len() {
            self.pos = RowPos { line: self.pos.line + 1, sub: 0 };
            self.current = line_rows(&self.lines[self.pos.line], self.width);
        } else {
            self.done = true;
        }
        Some(item)
    }
}

/// The row `count` rows above `pos` (or the first row of the document).
fn rows_above(lines: &[String], width: usize, mut pos: RowPos, count: usize) -> RowPos {
    for _ in 0..count {
        if pos.sub > 0 {
            pos.sub -= 1;
        } else if pos.line > 0 {
            pos.line -= 1;
            pos.sub = line_rows(&lines[pos.line], width).len() - 1;
        } else {
            break;
        }
    }
    pos
}

/// The row `count` rows below `pos` (or the last row of the document).
fn rows_below(lines: &[String], width: usize, pos: RowPos, count: usize) -> RowPos {
    Rows::new(lines, width, pos).take(count + 1).last().map_or(pos, |(p, _)| p)
}

/// Every operation here works on the rows it needs (at most a screenful),
/// never on the whole document, so large documents stay fast.
#[derive(Default)]
pub struct EditorView {
    /// First row on screen.
    top: RowPos,
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

    /// The cursor's row, that line's rows, and the cursor's x in the row.
    fn cursor_row(&self, buffer: &TextBuffer) -> (RowPos, Vec<(usize, usize)>, usize) {
        let cursor = buffer.cursor();
        let rows = line_rows(&buffer.lines()[cursor.line], self.wrap_width());
        let sub = sub_row(&rows, cursor.column);
        let x = cursor.column.saturating_sub(rows[sub].0);
        (RowPos { line: cursor.line, sub }, rows, x)
    }

    /// The cursor's x position within its screen row.
    pub fn cursor_x(&self, buffer: &TextBuffer) -> usize {
        self.cursor_row(buffer).2
    }

    /// The position `delta` screen rows above (negative) or below the cursor,
    /// as close to column `goal_x` as the target row allows.
    pub fn vertical(&self, buffer: &TextBuffer, delta: isize, goal_x: usize) -> Cursor {
        let (lines, width) = (buffer.lines(), self.wrap_width());
        let (pos, _, _) = self.cursor_row(buffer);
        let target = if delta < 0 {
            rows_above(lines, width, pos, delta.unsigned_abs())
        } else {
            rows_below(lines, width, pos, delta as usize)
        };
        let rows = line_rows(&lines[target.line], width);
        Cursor::new_at(target.line, rows[target.sub].0 + goal_x.min(max_x(&rows, target.sub)))
    }

    /// Start (`end == false`) or end of the cursor's screen row.
    pub fn row_edge(&self, buffer: &TextBuffer, end: bool) -> Cursor {
        let (pos, rows, _) = self.cursor_row(buffer);
        let x = if end { max_x(&rows, pos.sub) } else { 0 };
        Cursor::new_at(pos.line, rows[pos.sub].0 + x)
    }

    /// Adjust `top` so the cursor row is on screen.
    fn scroll_to(&mut self, lines: &[String], cursor: RowPos) {
        let width = self.wrap_width();
        // Edits may have removed the top line or some of its rows.
        let line = self.top.line.min(lines.len() - 1);
        let sub = self.top.sub.min(line_rows(&lines[line], width).len() - 1);
        self.top = RowPos { line, sub };

        if cursor < self.top {
            self.top = cursor;
        } else if !Rows::new(lines, width, self.top).take(self.height).any(|(p, _)| p == cursor) {
            self.top = rows_above(lines, width, cursor, self.height - 1);
        }
    }

    /// Draw the document; returns the cursor's screen position.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, buffer: &TextBuffer) -> Option<Position> {
        self.width = usize::from(area.width);
        self.height = usize::from(area.height);
        if area.is_empty() {
            return None;
        }
        let lines_text = buffer.lines();
        let (cursor_pos, _, cursor_x) = self.cursor_row(buffer);
        self.scroll_to(lines_text, cursor_pos);

        let selection = buffer.selection().filter(|s| !s.is_empty()).map(|s| s.normalize());
        // Write cells directly (no Paragraph): one char per cell, so screen
        // columns always equal cursor columns, and no per-frame grapheme
        // segmentation or span allocations.
        let base = super::base();
        let buf = frame.buffer_mut();
        buf.set_style(area, base);
        let mut cached: Option<(usize, Vec<char>, Vec<Style>)> = None;
        let mut cursor_y = 0;
        for (y, (pos, (start, end))) in Rows::new(lines_text, self.wrap_width(), self.top).take(self.height).enumerate() {
            if pos == cursor_pos {
                cursor_y = y;
            }
            if cached.as_ref().is_none_or(|(line, _, _)| *line != pos.line) {
                let text = &lines_text[pos.line];
                cached = Some((pos.line, text.chars().collect(), char_styles(text, pos.line, selection)));
            }
            let (_, chars, styles) = cached.as_ref().unwrap();
            let row_y = area.y + y as u16;
            for (x, (&ch, &style)) in chars[start..end].iter().zip(&styles[start..end]).take(self.width).enumerate() {
                let cell = &mut buf[(area.x + x as u16, row_y)];
                cell.set_char(if ch.is_control() { ' ' } else { ch });
                cell.set_style(style);
            }
        }

        let x = area.x + cursor_x as u16;
        let y = area.y + cursor_y as u16;
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
            // Keep bold/italic, but use the selection colours.
            let selected = super::selection();
            for style in styles.iter_mut().take(to).skip(from) {
                *style = style.patch(selected);
            }
        }
    }
    styles
}

fn style_for(kind: SpanKind) -> Style {
    match kind {
        SpanKind::Plain => Style::new(),
        SpanKind::Heading => super::heading(),
        SpanKind::Bold => super::bold(),
        SpanKind::Italic => super::italic(),
        SpanKind::BoldItalic => super::bold().add_modifier(Modifier::ITALIC),
        SpanKind::Marker(of) => {
            let span = style_for(of.span());
            match theme::current().marker {
                Some(color) => span.fg(color),
                None => span,
            }
        }
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
        // The short-line shortcut agrees with wrapping.
        assert_eq!(line_rows("héllo", 8), wrap_line(&chars("héllo"), 8));
        assert_eq!(line_rows("abcd", 4), vec![(0, 4), (4, 4)]);
    }

    #[test]
    fn cursor_maps_to_rows() {
        let rows = line_rows("hello world", 8);
        assert_eq!(sub_row(&rows, 3), 0);
        // The break position belongs to the next row.
        assert_eq!(sub_row(&rows, 6), 1);
        assert_eq!(sub_row(&rows, 11), 1);
        let lines = vec!["hello world".to_string(), "x".to_string()];
        let all: Vec<_> = Rows::new(&lines, 8, RowPos::default()).map(|(p, _)| (p.line, p.sub)).collect();
        assert_eq!(all, vec![(0, 0), (0, 1), (1, 0)]);
        let from_middle: Vec<_> = Rows::new(&lines, 8, RowPos { line: 0, sub: 1 }).map(|(_, r)| r).collect();
        assert_eq!(from_middle, vec![(6, 11), (0, 1)]);
        assert_eq!(rows_above(&lines, 8, RowPos { line: 1, sub: 0 }, 1), RowPos { line: 0, sub: 1 });
        assert_eq!(rows_above(&lines, 8, RowPos { line: 1, sub: 0 }, 9), RowPos::default());
        assert_eq!(rows_below(&lines, 8, RowPos::default(), 9), RowPos { line: 1, sub: 0 });
    }

    #[test]
    fn vertical_movement_follows_rows() {
        let mut buffer = TextBuffer::from_string("hello world\nx");
        let view = EditorView { top: RowPos::default(), width: 8, height: 10 };
        buffer.move_cursor(Cursor::new_at(0, 2));
        assert_eq!(view.vertical(&buffer, 1, 2), Cursor::new_at(0, 8));
        buffer.move_cursor(Cursor::new_at(0, 8));
        assert_eq!(view.vertical(&buffer, 1, 2), Cursor::new_at(1, 1));
        assert_eq!(view.vertical(&buffer, -1, 2), Cursor::new_at(0, 2));
        // End of a wrapped (non-last) row stops before the break.
        buffer.move_cursor(Cursor::new_at(0, 1));
        assert_eq!(view.row_edge(&buffer, true), Cursor::new_at(0, 5));
    }

    #[test]
    fn scrolling_keeps_the_cursor_on_screen() {
        let text: Vec<String> = (0..50).map(|i| alloc::format!("line {i}")).collect();
        let mut buffer = TextBuffer::from_string(&text.join("\n"));
        let mut view = EditorView { top: RowPos::default(), width: 20, height: 5 };
        let lines = buffer.lines().to_vec();
        // Moving below the screen scrolls so the cursor is the last row.
        view.scroll_to(&lines, RowPos { line: 10, sub: 0 });
        assert_eq!(view.top, RowPos { line: 6, sub: 0 });
        // Within the screen: no scrolling.
        view.scroll_to(&lines, RowPos { line: 8, sub: 0 });
        assert_eq!(view.top, RowPos { line: 6, sub: 0 });
        // Above the screen: the cursor row becomes the top.
        view.scroll_to(&lines, RowPos { line: 2, sub: 0 });
        assert_eq!(view.top, RowPos { line: 2, sub: 0 });
        // A top beyond the document (after deleting lines) is clamped.
        view.top = RowPos { line: 49, sub: 3 };
        buffer.move_cursor(Cursor::new_at(0, 0));
        view.scroll_to(&lines[..3], RowPos { line: 0, sub: 0 });
        assert_eq!(view.top, RowPos::default());
    }
}

