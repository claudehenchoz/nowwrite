use super::cursor::{Cursor, Selection};

#[derive(Debug, Clone)]
pub enum Operation {
    Insert { position: Cursor, text: String },
    Delete { position: Cursor, text: String },
    Replace { position: Cursor, old: String, new: String },
}

pub struct TextBuffer {
    lines: Vec<String>,
    cursor: Cursor,
    selection: Option<Selection>,
    undo_stack: Vec<Operation>,
    redo_stack: Vec<Operation>,
    modified: bool,
    file_path: Option<String>,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: Cursor::new(),
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            file_path: None,
        }
    }

    pub fn from_string(content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };

        Self {
            lines,
            cursor: Cursor::new(),
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            file_path: None,
        }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
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

    pub fn file_path(&self) -> Option<&String> {
        self.file_path.as_ref()
    }

    pub fn set_file_path(&mut self, path: String) {
        self.file_path = Some(path);
    }

    pub fn move_cursor(&mut self, new_cursor: Cursor) {
        self.cursor = self.clamp_cursor(new_cursor);
        self.selection = None;
    }

    pub fn move_cursor_with_selection(&mut self, new_cursor: Cursor) {
        let new_cursor = self.clamp_cursor(new_cursor);
        
        if let Some(selection) = self.selection {
            self.selection = Some(Selection::new(selection.start, new_cursor));
        } else {
            self.selection = Some(Selection::new(self.cursor, new_cursor));
        }
        
        self.cursor = new_cursor;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        
        let operation = Operation::Insert {
            position: self.cursor,
            text: ch.to_string(),
        };

        if ch == '\n' {
            let current_line = self.lines[self.cursor.line].clone();
            let (left, right) = current_line.split_at(self.cursor.column);
            
            self.lines[self.cursor.line] = left.to_string();
            self.lines.insert(self.cursor.line + 1, right.to_string());
            
            self.cursor.line += 1;
            self.cursor.column = 0;
        } else {
            self.lines[self.cursor.line].insert(self.cursor.column, ch);
            self.cursor.column += 1;
        }

        self.push_operation(operation);
        self.modified = true;
    }

    pub fn insert_string(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_char(ch);
        }
    }

    pub fn delete_char(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }

        if self.cursor.column > 0 {
            let ch = self.lines[self.cursor.line].remove(self.cursor.column - 1);
            self.cursor.column -= 1;
            
            let operation = Operation::Delete {
                position: self.cursor,
                text: ch.to_string(),
            };
            self.push_operation(operation);
            self.modified = true;
        } else if self.cursor.line > 0 {
            let current_line = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            self.cursor.column = self.lines[self.cursor.line].len();
            self.lines[self.cursor.line].push_str(&current_line);
            
            let operation = Operation::Delete {
                position: Cursor::new_at(self.cursor.line, self.cursor.column),
                text: "\n".to_string(),
            };
            self.push_operation(operation);
            self.modified = true;
        }
    }

    pub fn delete_selection(&mut self) {
        if let Some(selection) = self.selection {
            let selection = selection.normalize();
            
            if selection.start.line == selection.end.line {
                let line = &mut self.lines[selection.start.line];
                let deleted = line.drain(selection.start.column..selection.end.column).collect::<String>();
                
                let operation = Operation::Delete {
                    position: selection.start,
                    text: deleted,
                };
                self.push_operation(operation);
            } else {
                let mut deleted_text = String::new();
                
                let first_line = &mut self.lines[selection.start.line];
                deleted_text.push_str(&first_line[selection.start.column..]);
                first_line.truncate(selection.start.column);
                
                for line_idx in (selection.start.line + 1)..selection.end.line {
                    deleted_text.push('\n');
                    deleted_text.push_str(&self.lines[line_idx]);
                }
                
                if selection.end.line < self.lines.len() {
                    deleted_text.push('\n');
                    deleted_text.push_str(&self.lines[selection.end.line][..selection.end.column]);
                    
                    let remainder = self.lines[selection.end.line][selection.end.column..].to_string();
                    self.lines[selection.start.line].push_str(&remainder);
                }
                
                self.lines.drain((selection.start.line + 1)..=selection.end.line);
                
                let operation = Operation::Delete {
                    position: selection.start,
                    text: deleted_text,
                };
                self.push_operation(operation);
            }
            
            self.cursor = selection.start;
            self.selection = None;
            self.modified = true;
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(operation) = self.undo_stack.pop() {
            self.apply_reverse_operation(&operation);
            self.redo_stack.push(operation);
            self.modified = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(operation) = self.redo_stack.pop() {
            self.apply_operation(&operation);
            self.undo_stack.push(operation);
            self.modified = true;
            true
        } else {
            false
        }
    }

    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    fn clamp_cursor(&self, cursor: Cursor) -> Cursor {
        let line = cursor.line.min(self.lines.len().saturating_sub(1));
        let column = if line < self.lines.len() {
            cursor.column.min(self.lines[line].len())
        } else {
            0
        };
        Cursor::new_at(line, column)
    }

    fn push_operation(&mut self, operation: Operation) {
        self.undo_stack.push(operation);
        self.redo_stack.clear();
        
        if self.undo_stack.len() > 1000 {
            self.undo_stack.drain(0..500);
        }
    }

    fn apply_operation(&mut self, operation: &Operation) {
        match operation {
            Operation::Insert { position, text } => {
                self.cursor = *position;
                self.insert_string(text);
            }
            Operation::Delete { position, text: _ } => {
                self.cursor = *position;
                self.delete_char();
            }
            Operation::Replace { position, old: _, new } => {
                self.cursor = *position;
                self.insert_string(new);
            }
        }
    }

    fn apply_reverse_operation(&mut self, operation: &Operation) {
        match operation {
            Operation::Insert { position, text: _ } => {
                self.cursor = *position;
                self.delete_char();
            }
            Operation::Delete { position, text } => {
                self.cursor = *position;
                self.insert_string(text);
            }
            Operation::Replace { position, old, new: _ } => {
                self.cursor = *position;
                self.insert_string(old);
            }
        }
    }
}