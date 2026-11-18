pub mod editor_view;
pub mod modal;
pub mod status_bar;

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

// The palette is limited to what the UEFI text console can show: default
// grey text, bright white, dark grey (hints), and one grey background, which
// is reserved for the status bar, selection and the software cursor.

pub fn bright() -> Style {
    Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
}

/// Italic shows as bright text (the UEFI console has no italics), like bold.
pub fn italic() -> Style {
    Style::new().fg(Color::White).add_modifier(Modifier::ITALIC)
}

/// De-emphasized text such as hints.
pub fn muted() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub fn bar() -> Style {
    Style::new().fg(Color::White).bg(Color::DarkGray)
}

pub fn selected() -> Style {
    Style::new()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

/// A `width` x `height` rect centered in `area` (clamped to fit).
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(row);
    rect
}
