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

/// The kind of formatted text a span holds (or a marker belongs to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emphasis {
    Heading,
    Bold,
    Italic,
    /// `***text***`
    BoldItalic,
}

/// How a span of source text should be highlighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Plain,
    Heading,
    Bold,
    Italic,
    BoldItalic,
    /// The `#`, `*`, `**`, `***` or `_` characters around formatted text.
    Marker(Emphasis),
}

impl Emphasis {
    /// The kind of the text the marker surrounds.
    pub fn span(self) -> SpanKind {
        match self {
            Emphasis::Heading => SpanKind::Heading,
            Emphasis::Bold => SpanKind::Bold,
            Emphasis::Italic => SpanKind::Italic,
            Emphasis::BoldItalic => SpanKind::BoldItalic,
        }
    }
}

/// Split `line` into highlighted spans **including** the markdown markers
/// (as `Marker` spans), so the concatenated spans equal the input and screen
/// columns match cursor columns. Unclosed markers are left as plain text.
pub fn highlight(line: &str) -> Vec<(SpanKind, &str)> {
    if let Some(level) = MarkdownParser::get_heading_level(line) {
        let (marker, text) = line.split_at(usize::from(level));
        return alloc::vec![(SpanKind::Marker(Emphasis::Heading), marker), (SpanKind::Heading, text)];
    }

    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut i = 0;
    'scan: while i < line.len() {
        let rest = &line[i..];
        let candidates: &[(&str, Emphasis)] = if rest.starts_with('*') {
            &[("***", Emphasis::BoldItalic), ("**", Emphasis::Bold), ("*", Emphasis::Italic)]
        } else if rest.starts_with('_') {
            &[("_", Emphasis::Italic)]
        } else {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };

        // The longest marker that has a closing partner with text between.
        for &(marker, emphasis) in candidates {
            if !rest.starts_with(marker) {
                continue;
            }
            let body = &rest[marker.len()..];
            if let Some(close) = body.find(marker).filter(|&c| c > 0) {
                if plain_start < i {
                    spans.push((SpanKind::Plain, &line[plain_start..i]));
                }
                let text_end = i + marker.len() + close;
                let end = text_end + marker.len();
                spans.push((SpanKind::Marker(emphasis), &line[i..i + marker.len()]));
                spans.push((emphasis.span(), &line[i + marker.len()..text_end]));
                spans.push((SpanKind::Marker(emphasis), &line[text_end..end]));
                i = end;
                plain_start = end;
                continue 'scan;
            }
        }
        // Unmatched: skip the whole marker run so `**` isn't retried as `*`.
        i += rest.len() - rest.trim_start_matches(rest.as_bytes()[0] as char).len();
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

    use SpanKind::{Marker, Plain};

    fn joined(spans: &[(SpanKind, &str)]) -> String {
        spans.iter().map(|(_, t)| *t).collect()
    }

    #[test]
    fn highlight_headings() {
        assert_eq!(highlight("## Title"), [(Marker(Emphasis::Heading), "##"), (SpanKind::Heading, " Title")]);
        // Not headings: no space, or more than six #.
        assert_eq!(highlight("#tag"), [(Plain, "#tag")]);
        assert_eq!(highlight("####### x"), [(Plain, "####### x")]);
    }

    #[test]
    fn highlight_emphasis_with_markers() {
        assert_eq!(
            highlight("a *it* b"),
            [(Plain, "a "), (Marker(Emphasis::Italic), "*"), (SpanKind::Italic, "it"), (Marker(Emphasis::Italic), "*"), (Plain, " b")]
        );
        assert_eq!(
            highlight("**bold**"),
            [(Marker(Emphasis::Bold), "**"), (SpanKind::Bold, "bold"), (Marker(Emphasis::Bold), "**")]
        );
        assert_eq!(
            highlight("***both***!"),
            [
                (Marker(Emphasis::BoldItalic), "***"),
                (SpanKind::BoldItalic, "both"),
                (Marker(Emphasis::BoldItalic), "***"),
                (Plain, "!"),
            ]
        );
        assert_eq!(highlight("_u_")[1], (SpanKind::Italic, "u"));
    }

    #[test]
    fn highlight_unmatched_markers_stay_plain() {
        for line in ["*open é", "a ** b", "x * y", "**", "*"] {
            assert_eq!(highlight(line), [(Plain, line)], "{line}");
        }
        // An unclosed `***` falls back to the longest marker that closes.
        assert_eq!(
            highlight("***a**"),
            [(Marker(Emphasis::Bold), "**"), (SpanKind::Bold, "*a"), (Marker(Emphasis::Bold), "**")]
        );
    }

    #[test]
    fn highlight_spans_concatenate_to_input() {
        let line = "a **bold** and *it* plus ***bi*** and *open é ## x";
        assert_eq!(joined(&highlight(line)), line);
    }
}
