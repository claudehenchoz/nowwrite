pub mod editor_view;
pub mod modal;
pub mod status_bar;
pub mod theme;

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};
use theme::{current, Pair};

// Every style comes from the active theme (`theme::current()`). UI chrome
// uses only ASCII, box drawing and arrows, so the UEFI text console can
// show it; documents may contain anything.

fn pair(p: Pair) -> Style {
    Style::new().fg(p.fg).bg(p.bg)
}

/// Plain document text on the editor background.
pub fn base() -> Style {
    let t = current();
    Style::new().fg(t.text).bg(t.background)
}

pub fn heading() -> Style {
    Style::new().fg(current().heading).add_modifier(Modifier::BOLD)
}

pub fn bold() -> Style {
    Style::new().fg(current().bold).add_modifier(Modifier::BOLD)
}

pub fn italic() -> Style {
    Style::new().fg(current().italic).add_modifier(Modifier::ITALIC)
}

/// De-emphasized text such as hints.
pub fn muted() -> Style {
    Style::new().fg(current().hint)
}

pub fn selection() -> Style {
    pair(current().selection)
}

/// The software block cursor.
pub fn cursor() -> Style {
    pair(current().cursor)
}

pub fn bar() -> Style {
    pair(current().bar)
}

/// Key hints inside the status bar.
pub fn bar_key() -> Style {
    Style::new().fg(current().bar_key)
}

/// Body text inside a dialog.
pub fn dialog() -> Style {
    pair(current().dialog)
}

pub fn dialog_title() -> Style {
    Style::new().fg(current().dialog_title).add_modifier(Modifier::BOLD)
}

/// Highlighted row of a dialog's list.
pub fn dialog_selected() -> Style {
    pair(current().dialog_selected)
}

pub fn directory() -> Style {
    Style::new().fg(current().directory)
}

pub fn error() -> Style {
    Style::new().fg(current().error)
}

/// A themed modal frame with a centered title and a key hint in the bottom
/// border.
pub fn dialog_block<'a>(title: &'a str, hint: &'a str) -> Block<'a> {
    let t = current();
    Block::bordered()
        .border_type(if t.rounded { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::new().fg(t.dialog_border))
        .style(dialog())
        .title(Line::from(title).centered().style(dialog_title()))
        .title_bottom(Line::from(hint).centered().style(Style::new().fg(t.dialog.fg)))
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
