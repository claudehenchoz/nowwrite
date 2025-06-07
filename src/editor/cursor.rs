#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { line: 0, column: 0 }
    }

    pub fn new_at(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: Cursor,
    pub end: Cursor,
}

impl Selection {
    pub fn new(start: Cursor, end: Cursor) -> Self {
        Self { start, end }
    }

    pub fn normalize(&self) -> Self {
        if self.start.line < self.end.line 
            || (self.start.line == self.end.line && self.start.column <= self.end.column) {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}