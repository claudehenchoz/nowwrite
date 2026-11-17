use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownToken {
    Heading { level: u8, text: String },
    Bold(String),
    Italic(String),
    PlainText(String),
}

#[derive(Debug, Clone)]
pub struct MarkdownLine {
    pub tokens: Vec<MarkdownToken>,
}

impl MarkdownLine {
    pub fn parse(line: &str) -> Self {
        let mut tokens = Vec::new();
        let mut chars = line.chars().peekable();
        let mut current_text = String::new();

        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count() as u8;
            if level <= 6 && line.chars().nth(level as usize) == Some(' ') {
                let text = line[level as usize + 1..].to_string();
                tokens.push(MarkdownToken::Heading { level, text });
                return Self { tokens };
            }
        }

        while let Some(ch) = chars.next() {
            match ch {
                '*' => {
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        if !current_text.is_empty() {
                            tokens.push(MarkdownToken::PlainText(current_text.clone()));
                            current_text.clear();
                        }
                        
                        let bold_text = Self::parse_until(&mut chars, "**");
                        if !bold_text.is_empty() {
                            tokens.push(MarkdownToken::Bold(bold_text));
                        }
                    } else {
                        if !current_text.is_empty() {
                            tokens.push(MarkdownToken::PlainText(current_text.clone()));
                            current_text.clear();
                        }
                        
                        let italic_text = Self::parse_until(&mut chars, "*");
                        if !italic_text.is_empty() {
                            tokens.push(MarkdownToken::Italic(italic_text));
                        }
                    }
                }
                '_' => {
                    if !current_text.is_empty() {
                        tokens.push(MarkdownToken::PlainText(current_text.clone()));
                        current_text.clear();
                    }
                    
                    let italic_text = Self::parse_until(&mut chars, "_");
                    if !italic_text.is_empty() {
                        tokens.push(MarkdownToken::Italic(italic_text));
                    }
                }
                _ => {
                    current_text.push(ch);
                }
            }
        }

        if !current_text.is_empty() {
            tokens.push(MarkdownToken::PlainText(current_text));
        }

        if tokens.is_empty() {
            tokens.push(MarkdownToken::PlainText(line.to_string()));
        }

        Self { tokens }
    }

    fn parse_until<I>(chars: &mut core::iter::Peekable<I>, delimiter: &str) -> String
    where
        I: Iterator<Item = char>,
    {
        let mut text = String::new();
        let delimiter_chars: Vec<char> = delimiter.chars().collect();
        let mut match_count = 0;

        while let Some(ch) = chars.next() {
            if ch == delimiter_chars[match_count] {
                match_count += 1;
                if match_count == delimiter_chars.len() {
                    return text;
                }
            } else {
                for _ in 0..match_count {
                    text.push(delimiter_chars[0]);
                }
                match_count = 0;
                text.push(ch);
            }
        }

        for _ in 0..match_count {
            text.push(delimiter_chars[0]);
        }
        text
    }
}

/// How a span of source text should be highlighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Plain,
    Heading,
    Bold,
    Italic,
}

/// Split `line` into highlighted spans **including** the markdown markers,
/// so the concatenated spans equal the input and screen columns match
/// cursor columns. Unclosed markers are left as plain text.
pub fn highlight(line: &str) -> Vec<(SpanKind, &str)> {
    if MarkdownParser::get_heading_level(line).is_some() {
        return alloc::vec![(SpanKind::Heading, line)];
    }

    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let (marker, kind) = if rest.starts_with("**") {
            ("**", SpanKind::Bold)
        } else if rest.starts_with('*') {
            ("*", SpanKind::Italic)
        } else if rest.starts_with('_') {
            ("_", SpanKind::Italic)
        } else {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };

        let body = &rest[marker.len()..];
        match body.find(marker) {
            Some(close) if close > 0 => {
                if plain_start < i {
                    spans.push((SpanKind::Plain, &line[plain_start..i]));
                }
                let end = i + marker.len() * 2 + close;
                spans.push((kind, &line[i..end]));
                i = end;
                plain_start = end;
            }
            _ => i += marker.len(),
        }
    }
    if plain_start < line.len() || spans.is_empty() {
        spans.push((SpanKind::Plain, &line[plain_start..]));
    }
    spans
}

pub struct MarkdownParser;

impl MarkdownParser {
    pub fn parse_lines(lines: &[String]) -> Vec<MarkdownLine> {
        lines.iter().map(|line| MarkdownLine::parse(line)).collect()
    }

    pub fn is_heading(line: &str) -> bool {
        line.starts_with('#') && line.chars().take_while(|&c| c == '#').count() <= 6
    }

    pub fn get_heading_level(line: &str) -> Option<u8> {
        if Self::is_heading(line) {
            let level = line.chars().take_while(|&c| c == '#').count() as u8;
            if level <= 6 && line.chars().nth(level as usize) == Some(' ') {
                Some(level)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_parsing() {
        let line = MarkdownLine::parse("# Heading 1");
        assert_eq!(line.tokens.len(), 1);
        match &line.tokens[0] {
            MarkdownToken::Heading { level, text } => {
                assert_eq!(*level, 1);
                assert_eq!(text, "Heading 1");
            }
            _ => panic!("Expected heading token"),
        }
    }

    #[test]
    fn test_bold_parsing() {
        let line = MarkdownLine::parse("This is **bold** text");
        assert_eq!(line.tokens.len(), 3);
        assert!(matches!(line.tokens[0], MarkdownToken::PlainText(_)));
        assert!(matches!(line.tokens[1], MarkdownToken::Bold(_)));
        assert!(matches!(line.tokens[2], MarkdownToken::PlainText(_)));
    }

    #[test]
    fn test_italic_parsing() {
        let line = MarkdownLine::parse("This is *italic* text");
        assert_eq!(line.tokens.len(), 3);
        assert!(matches!(line.tokens[0], MarkdownToken::PlainText(_)));
        assert!(matches!(line.tokens[1], MarkdownToken::Italic(_)));
        assert!(matches!(line.tokens[2], MarkdownToken::PlainText(_)));
    }

    #[test]
    fn highlight_keeps_markers() {
        let line = "a **bold** and *it* plus *open é";
        let spans = highlight(line);
        let joined: String = spans.iter().map(|(_, t)| *t).collect();
        assert_eq!(joined, line);
        assert!(spans.contains(&(SpanKind::Bold, "**bold**")));
        assert!(spans.contains(&(SpanKind::Italic, "*it*")));
        assert_eq!(highlight("## Title"), [(SpanKind::Heading, "## Title")]);
    }
}
