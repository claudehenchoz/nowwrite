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

    fn parse_until<I>(chars: &mut std::iter::Peekable<I>, delimiter: &str) -> String
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
}