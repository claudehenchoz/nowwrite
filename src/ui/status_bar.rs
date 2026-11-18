use crate::editor::buffer::TextBuffer;
use crate::file::file_name;
use crate::platform::BatteryStatus;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Extra state shown on the right of the status bar.
pub struct StatusInfo<'a> {
    /// Transient message (saved, errors, hints) until the next key.
    pub message: Option<&'a str>,
    /// Active keyboard layout code, where NowWrite applies layouts itself.
    pub layout: Option<&'a str>,
    /// Accent of a dead key waiting for its letter.
    pub pending_dead: Option<char>,
    pub battery: Option<BatteryStatus>,
}

/// Bottom row. Left: document name, unsaved state and counts. Right:
/// message, pending accent, layout, cursor position, battery, help hint.
pub fn render(frame: &mut Frame, area: Rect, buffer: &TextBuffer, info: &StatusInfo) {
    let name = buffer.file_path().map_or("[new document]", file_name);
    let modified = if buffer.is_modified() { " (modified)" } else { "" };
    let words = buffer.word_count();
    let short_left = format!(" {name}{modified}");
    let full_left = format!(
        "{short_left} | {words} {} | {} chars",
        if words == 1 { "word" } else { "words" },
        buffer.char_count()
    );

    let cursor = buffer.cursor();
    let mut parts: Vec<String> = Vec::new();
    if let Some(message) = info.message {
        parts.push(String::from(message));
    }
    if let Some(accent) = info.pending_dead {
        parts.push(format!("{accent} + letter"));
    }
    if let Some(layout) = info.layout {
        parts.push(String::from(layout));
    }
    parts.push(format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1));
    if let Some(b) = info.battery {
        parts.push(format!("{}%{}", b.level, if b.charging { "+" } else { "" }));
    }
    parts.push(String::from("F1 Help "));
    let right = parts.join(" | ");

    // While a message is showing (or space runs out), the counts step aside.
    let width = usize::from(area.width);
    let right_len = right.chars().count();
    let fits = full_left.chars().count() + right_len + 2 <= width;
    let left = if info.message.is_none() && fits { full_left } else { short_left };

    let right_width = (right_len as u16).min(area.width);
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_width)]).areas(area);

    let bar = super::bar();
    frame.render_widget(Paragraph::new(Line::from(left)).style(bar), left_area);
    frame.render_widget(Paragraph::new(Line::from(right)).style(bar), right_area);
}
