use super::cursor::{Cursor, Selection};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// An edit as recorded for undo/redo. `position` is where `text` starts;
/// a `'\n'` in `text` stands for a line break.
#[derive(Debug, Clone)]
enum Operation {
    Insert { position: Cursor, text: String },
    Delete { position: Cursor, text: String },
}

/// Text stored as lines. Cursor columns are **char** indices, not bytes.
pub struct TextBuffer {
    lines: Vec<String>,
    cursor: Cursor,
    selection: Option<Selection>,
    undo_stack: Vec<Operation>,
    redo_stack: Vec<Operation>,
    modified: bool,
    file_path: Option<String>,
    /// Whether the next typed text may join the last undo step
    /// (consecutive typing, no cursor movement in between).
    merge_open: bool,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Byte offset of char index `col` in `line` (clamped to the end).
fn byte_idx(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
}

impl TextBuffer {
    pub fn new() -> Self {
        Self::from_string("")
    }

    pub fn from_string(content: &str) -> Self {
        let mut lines: Vec<String> = content
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
            .collect();
        if lines.is_empty() {
            lines = vec![String::new()];
        }

        Self {
            lines,
            cursor: Cursor::new(),
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            file_path: None,
            merge_open: false,
        }
    }

    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Length of line `idx` in chars.
    pub fn line_len(&self, idx: usize) -> usize {
        self.lines.get(idx).map_or(0, |l| l.chars().count())
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    pub fn set_file_path(&mut self, path: String) {
        self.file_path = Some(path);
    }

    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    pub fn word_count(&self) -> usize {
        self.lines.iter().map(|l| l.split_whitespace().count()).sum()
    }

    pub fn char_count(&self) -> usize {
        let chars: usize = self.lines.iter().map(|l| l.chars().count()).sum();
        chars + self.lines.len() - 1
    }

    pub fn move_cursor(&mut self, new_cursor: Cursor) {
        self.cursor = self.clamp_cursor(new_cursor);
        self.selection = None;
        self.merge_open = false;
    }

    pub fn move_cursor_with_selection(&mut self, new_cursor: Cursor) {
        self.merge_open = false;
        let new_cursor = self.clamp_cursor(new_cursor);
        let anchor = self.selection.map_or(self.cursor, |s| s.start);
        self.selection = Some(Selection::new(anchor, new_cursor));
        self.cursor = new_cursor;
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut buf = [0u8; 4];
        self.insert_str(ch.encode_utf8(&mut buf));
    }

    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.delete_selection();
        let position = self.cursor;
        self.cursor = self.raw_insert(position, text);

        // Group typing into words for undo: keep appending to the last insert
        // while it's contiguous, but start a new step at the first letter
        // after whitespace or at a line break.
        let merge = self.merge_open
            && !text.contains('\n')
            && match self.undo_stack.last() {
                Some(Operation::Insert { position: p, text: t }) => {
                    !t.contains('\n')
                        && p.line == position.line
                        && p.column + t.chars().count() == position.column
                        && !(t.ends_with(char::is_whitespace) && !text.starts_with(char::is_whitespace))
                }
                _ => false,
            };
        if merge {
            if let Some(Operation::Insert { text: t, .. }) = self.undo_stack.last_mut() {
                t.push_str(text);
            }
            self.redo_stack.clear();
        } else {
            self.record(Operation::Insert { position, text: text.to_string() });
        }
        self.merge_open = !text.contains('\n');
    }

    /// Backspace: delete the selection, or the char before the cursor.
    pub fn delete_char(&mut self) {
        if self.delete_selection() {
            return;
        }
        let position = if self.cursor.column > 0 {
            Cursor::new_at(self.cursor.line, self.cursor.column - 1)
        } else if self.cursor.line > 0 {
            Cursor::new_at(self.cursor.line - 1, self.line_len(self.cursor.line - 1))
        } else {
            return;
        };
        let text = self.raw_remove(position, 1);
        self.cursor = position;
        self.record(Operation::Delete { position, text });
    }

    /// Delete key: delete the selection, or the char under the cursor.
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let position = self.cursor;
        let text = self.raw_remove(position, 1);
        if !text.is_empty() {
            self.record(Operation::Delete { position, text });
        }
    }

    /// Delete the selected text. Returns whether anything was selected.
    pub fn delete_selection(&mut self) -> bool {
        let Some(selection) = self.selection.take() else {
            return false;
        };
        if selection.is_empty() {
            return false;
        }
        let Selection { start, end } = selection.normalize();
        let count = self.chars_between(start, end);
        if count > 0 {
            let text = self.raw_remove(start, count);
            self.record(Operation::Delete { position: start, text });
        }
        self.cursor = start;
        true
    }

    /// Bold/italic toggle. Surround the selection, or the word under the
    /// cursor, with `marker` (`*` or `**`), or remove the marker if it is
    /// already there. With neither, insert a marker pair and place the
    /// cursor between them.
    ///
    /// The cursor stays at the same place in the word (and a selection stays
    /// on the same text), so toggling again undoes the change.
    pub fn toggle_wrap(&mut self, marker: &str) {
        self.merge_open = false;
        let marker_len = marker.chars().count();
        let original = self.cursor;
        let selection = self.selection.take().filter(|sel| !sel.is_empty());
        let range = match selection {
            Some(sel) => {
                let sel = sel.normalize();
                Some((sel.start, sel.end))
            }
            None => self.word_at_cursor(),
        };

        let Some((start, end)) = range else {
            let start = self.cursor;
            self.insert_str(marker);
            self.insert_str(marker);
            self.cursor = Cursor::new_at(start.line, start.column + marker_len);
            self.merge_open = false;
            return;
        };

        // `*` markers present on both sides. Italic is 1 (or 3 = bold
        // italic); bold is 2 or more.
        let stars = self.stars_before(start).min(self.stars_after(end));
        let present = if marker_len >= 2 { stars >= 2 } else { stars == 1 || stars >= 3 };
        // Positions on the first line after the opening marker move by `delta`
        // columns once it is added (+) or removed (-).
        let shift = |pos: Cursor, delta: isize| {
            if pos.line == start.line && pos.column >= start.column {
                Cursor::new_at(pos.line, pos.column.saturating_add_signed(delta))
            } else {
                pos
            }
        };
        let delta = marker_len as isize;

        let delta = if present {
            // Remove the closing marker first so `start` stays valid.
            let text = self.raw_remove(end, marker_len);
            self.record(Operation::Delete { position: end, text });
            let open = Cursor::new_at(start.line, start.column - marker_len);
            let text = self.raw_remove(open, marker_len);
            self.record(Operation::Delete { position: open, text });
            -delta
        } else {
            // Closing marker first; the cursor and `start` stay valid.
            self.cursor = end;
            self.insert_str(marker);
            self.cursor = start;
            self.merge_open = false;
            self.insert_str(marker);
            delta
        };

        if selection.is_some() {
            // Keep the same text selected (without the markers).
            let (new_start, new_end) = (shift(start, delta), shift(end, delta));
            self.cursor = new_end;
            self.selection = Some(Selection::new(new_start, new_end));
        } else {
            self.cursor = shift(original, delta);
        }
        self.merge_open = false;
    }

    fn stars_before(&self, pos: Cursor) -> usize {
        let line: Vec<char> = self.lines[pos.line].chars().collect();
        line[..pos.column.min(line.len())].iter().rev().take_while(|&&c| c == '*').count()
    }

    fn stars_after(&self, pos: Cursor) -> usize {
        self.lines[pos.line].chars().skip(pos.column).take_while(|&c| c == '*').count()
    }

    pub fn undo(&mut self) -> bool {
        let Some(operation) = self.undo_stack.pop() else {
            return false;
        };
        match &operation {
            Operation::Insert { position, text } => {
                self.raw_remove(*position, text.chars().count());
                self.cursor = *position;
            }
            Operation::Delete { position, text } => {
                self.cursor = self.raw_insert(*position, text);
            }
        }
        self.redo_stack.push(operation);
        self.selection = None;
        self.merge_open = false;
        self.modified = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(operation) = self.redo_stack.pop() else {
            return false;
        };
        match &operation {
            Operation::Insert { position, text } => {
                self.cursor = self.raw_insert(*position, text);
            }
            Operation::Delete { position, text } => {
                self.raw_remove(*position, text.chars().count());
                self.cursor = *position;
            }
        }
        self.undo_stack.push(operation);
        self.selection = None;
        self.merge_open = false;
        self.modified = true;
        true
    }

    fn word_at_cursor(&self) -> Option<(Cursor, Cursor)> {
        let line: Vec<char> = self.lines[self.cursor.line].chars().collect();
        let col = self.cursor.column;
        let is_word = |c: char| c.is_alphanumeric() || c == '\'' || c == '-';
        let touches_word = (col < line.len() && is_word(line[col]))
            || (col > 0 && is_word(line[col - 1]));
        if !touches_word {
            return None;
        }
        let mut start = col;
        while start > 0 && is_word(line[start - 1]) {
            start -= 1;
        }
        let mut end = col;
        while end < line.len() && is_word(line[end]) {
            end += 1;
        }
        Some((
            Cursor::new_at(self.cursor.line, start),
            Cursor::new_at(self.cursor.line, end),
        ))
    }

    fn chars_between(&self, start: Cursor, end: Cursor) -> usize {
        if start.line == end.line {
            return end.column.saturating_sub(start.column);
        }
        let mut count = self.line_len(start.line) - start.column + 1;
        for line in (start.line + 1)..end.line {
            count += self.line_len(line) + 1;
        }
        count + end.column
    }

    /// Insert `text` at `pos` without recording it. Returns the cursor
    /// position just after the inserted text.
    fn raw_insert(&mut self, pos: Cursor, text: &str) -> Cursor {
        let mut cur = self.clamp_cursor(pos);
        for ch in text.chars() {
            let line = &mut self.lines[cur.line];
            let at = byte_idx(line, cur.column);
            if ch == '\n' {
                let rest = line.split_off(at);
                self.lines.insert(cur.line + 1, rest);
                cur = Cursor::new_at(cur.line + 1, 0);
            } else {
                line.insert(at, ch);
                cur.column += 1;
            }
        }
        self.modified = true;
        cur
    }

    /// Remove up to `count` chars starting at `pos` (line breaks count as
    /// one char) without recording it. Returns the removed text.
    fn raw_remove(&mut self, pos: Cursor, count: usize) -> String {
        let pos = self.clamp_cursor(pos);
        let mut removed = String::new();
        for _ in 0..count {
            if pos.column < self.line_len(pos.line) {
                let line = &mut self.lines[pos.line];
                let at = byte_idx(line, pos.column);
                removed.push(line.remove(at));
            } else if pos.line + 1 < self.lines.len() {
                let next = self.lines.remove(pos.line + 1);
                self.lines[pos.line].push_str(&next);
                removed.push('\n');
            } else {
                break;
            }
        }
        if !removed.is_empty() {
            self.modified = true;
        }
        removed
    }

    fn clamp_cursor(&self, cursor: Cursor) -> Cursor {
        let line = cursor.line.min(self.lines.len() - 1);
        Cursor::new_at(line, cursor.column.min(self.line_len(line)))
    }

    fn record(&mut self, operation: Operation) {
        self.merge_open = false;
        self.undo_stack.push(operation);
        self.redo_stack.clear();
        if self.undo_stack.len() > 1000 {
            self.undo_stack.drain(0..500);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_delete_non_ascii() {
        let mut buf = TextBuffer::new();
        for ch in "Grüße é".chars() {
            buf.insert_char(ch);
        }
        assert_eq!(buf.to_text(), "Grüße é");
        assert_eq!(buf.cursor().column, 7);
        buf.move_cursor(Cursor::new_at(0, 3));
        buf.insert_char('ö');
        assert_eq!(buf.to_text(), "Grüöße é");
        buf.delete_char();
        buf.delete_char();
        assert_eq!(buf.to_text(), "Grße é");
        buf.delete_forward();
        assert_eq!(buf.to_text(), "Gre é");
    }

    #[test]
    fn newline_split_and_join() {
        let mut buf = TextBuffer::from_string("äbc");
        buf.move_cursor(Cursor::new_at(0, 1));
        buf.insert_char('\n');
        assert_eq!(buf.lines(), ["ä", "bc"]);
        buf.delete_char();
        assert_eq!(buf.to_text(), "äbc");
        assert_eq!(buf.cursor(), Cursor::new_at(0, 1));
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut buf = TextBuffer::new();
        buf.insert_str("hé");
        buf.insert_char('\n');
        buf.insert_str("x");
        buf.delete_char();
        assert_eq!(buf.to_text(), "hé\n");
        assert!(buf.undo());
        assert_eq!(buf.to_text(), "hé\nx");
        assert!(buf.undo());
        assert!(buf.undo());
        assert_eq!(buf.to_text(), "hé");
        assert!(buf.redo());
        assert!(buf.redo());
        assert_eq!(buf.to_text(), "hé\nx");
    }

    #[test]
    fn delete_multiline_selection() {
        let mut buf = TextBuffer::from_string("one\ntwo\nthree");
        buf.move_cursor(Cursor::new_at(0, 1));
        buf.move_cursor_with_selection(Cursor::new_at(2, 2));
        buf.delete_char();
        assert_eq!(buf.to_text(), "oree");
        buf.undo();
        assert_eq!(buf.to_text(), "one\ntwo\nthree");
    }

    #[test]
    fn toggle_wrap_word_and_empty() {
        let mut buf = TextBuffer::from_string("say hello now");
        buf.move_cursor(Cursor::new_at(0, 6));
        buf.toggle_wrap("**");
        assert_eq!(buf.to_text(), "say **hello** now");
        // The cursor stays on the same letter ("l"), inside the markers...
        assert_eq!(buf.cursor(), Cursor::new_at(0, 8));
        // ...so toggling again removes them and puts it back.
        buf.toggle_wrap("**");
        assert_eq!(buf.to_text(), "say hello now");
        assert_eq!(buf.cursor(), Cursor::new_at(0, 6));

        let mut buf = TextBuffer::new();
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "**");
        assert_eq!(buf.cursor().column, 1);
    }

    #[test]
    fn toggle_repeats_with_cursor_at_word_edges() {
        // Cursor right after the word (typical right after typing it).
        let mut buf = TextBuffer::from_string("hello");
        buf.move_cursor(Cursor::new_at(0, 5));
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "*hello*");
        assert_eq!(buf.cursor(), Cursor::new_at(0, 6));
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "hello");
        assert_eq!(buf.cursor(), Cursor::new_at(0, 5));

        // Cursor at the start of the word.
        buf.move_cursor(Cursor::new_at(0, 0));
        buf.toggle_wrap("**");
        assert_eq!(buf.to_text(), "**hello**");
        assert_eq!(buf.cursor(), Cursor::new_at(0, 2));
        buf.toggle_wrap("**");
        assert_eq!(buf.to_text(), "hello");
        assert_eq!(buf.cursor(), Cursor::new_at(0, 0));
    }

    #[test]
    fn toggle_italic_and_bold_combine() {
        let mut buf = TextBuffer::from_string("**bold**");
        buf.move_cursor(Cursor::new_at(0, 4));
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "***bold***");
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "**bold**");
        buf.toggle_wrap("**");
        assert_eq!(buf.to_text(), "bold");
    }

    #[test]
    fn toggle_wrap_selection_repeats_and_undo() {
        let mut buf = TextBuffer::from_string("one two three");
        buf.move_cursor(Cursor::new_at(0, 4));
        buf.move_cursor_with_selection(Cursor::new_at(0, 13));
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "one *two three*");
        // The same text stays selected, so toggling again removes the markers.
        buf.toggle_wrap("*");
        assert_eq!(buf.to_text(), "one two three");
        assert_eq!(
            buf.selection().map(|s| s.normalize()),
            Some(Selection::new(Cursor::new_at(0, 4), Cursor::new_at(0, 13)))
        );
        buf.undo();
        buf.undo();
        assert_eq!(buf.to_text(), "one *two three*");
        buf.undo();
        buf.undo();
        assert_eq!(buf.to_text(), "one two three");
    }

    #[test]
    fn undo_groups_typing_by_word() {
        let mut buf = TextBuffer::new();
        for ch in "hello w\u{f6}rld".chars() {
            buf.insert_char(ch);
        }
        buf.undo();
        assert_eq!(buf.to_text(), "hello ");
        buf.undo();
        assert_eq!(buf.to_text(), "");
        buf.redo();
        buf.redo();
        assert_eq!(buf.to_text(), "hello w\u{f6}rld");

        // Moving the cursor starts a new undo step.
        let mut buf = TextBuffer::new();
        buf.insert_str("ab");
        buf.move_cursor(Cursor::new_at(0, 1));
        buf.insert_char('x');
        buf.undo();
        assert_eq!(buf.to_text(), "ab");
    }
}
