pub mod editor_view;
pub mod modal;
pub mod status_bar;

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

// The palette is limited to what the UEFI text console can show:
// default/grey text, bright white, and one grey background.

pub fn bright() -> Style {
    Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
}

pub fn italic() -> Style {
    Style::new().bg(Color::DarkGray)
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
