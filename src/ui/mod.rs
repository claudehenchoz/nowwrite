pub mod editor_view;
pub mod modal;
pub mod status_bar;
pub mod theme;

use ratatui::layout::Rect;
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

/// A `width` x `height` rect centered in `area` (clamped to fit). Plain
/// arithmetic: ratatui's `Layout` runs a constraint solver on every call
/// (its cache needs `std`), which is slow on UEFI.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let (width, height) = (width.min(area.width), height.min(area.height));
    // Odd leftovers put the extra cell before the box, like ratatui.
    let offset = |extra: u16| extra.div_ceil(2);
    Rect::new(area.x + offset(area.width - width), area.y + offset(area.height - height), width, height)
}

/// `area` split into everything but the last row, and the last row.
pub fn split_last_row(area: Rect) -> (Rect, Rect) {
    let top = Rect { height: area.height.saturating_sub(1), ..area };
    let bottom = Rect { y: area.y + top.height, height: area.height - top.height, ..area };
    (top, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::{Constraint, Flex, Layout};

    #[test]
    fn centered_matches_ratatui_layout() {
        for (aw, ah) in [(80, 25), (91, 25), (101, 25), (79, 24), (20, 5)] {
            let area = Rect::new(0, 0, aw, ah);
            for w in [1, 44, 50, 54, 60, 66, 100, 120] {
                for h in [1, 6, 7, 9, 16, 20, 30] {
                    let [row] = Layout::vertical([Constraint::Length(h.min(ah))]).flex(Flex::Center).areas(area);
                    let [rect] = Layout::horizontal([Constraint::Length(w.min(aw))]).flex(Flex::Center).areas(row);
                    assert_eq!(centered(area, w, h), rect, "area {aw}x{ah} box {w}x{h}");
                }
            }
        }
    }

    #[test]
    fn split_last_row_matches_ratatui_layout() {
        let area = Rect::new(0, 0, 91, 25);
        let [main, status] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        assert_eq!(split_last_row(area), (main, status));
    }
}
