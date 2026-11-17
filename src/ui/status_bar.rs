use crate::editor::buffer::TextBuffer;
use crate::file::file_name;
use crate::platform::BatteryStatus;
use alloc::format;
use alloc::string::String;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Bottom row: file name, counts and saved state on the left; a transient
/// message, cursor position and battery on the right.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    buffer: &TextBuffer,
    battery: Option<BatteryStatus>,
    message: Option<&str>,
) {
    let name = buffer.file_path().map_or("untitled.md", file_name);
    let modified = if buffer.is_modified() { " *" } else { "" };
    let left = format!(
        " {name}{modified} | {} words | {} chars",
        buffer.word_count(),
        buffer.char_count()
    );

    let cursor = buffer.cursor();
    let mut right = String::new();
    if let Some(message) = message {
        right.push_str(message);
        right.push_str(" | ");
    }
    right.push_str(&format!("{}:{}", cursor.line + 1, cursor.column + 1));
    if let Some(b) = battery {
        right.push_str(&format!(" | {}%{}", b.level, if b.charging { "+" } else { "" }));
    }
    right.push(' ');

    let right_width = right.chars().count() as u16;
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_width)]).areas(area);

    let bar = super::bar();
    frame.render_widget(Paragraph::new(Line::from(left)).style(bar), left_area);
    frame.render_widget(Paragraph::new(Line::from(right)).style(bar), right_area);
}
