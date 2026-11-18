//! The ratatui backend for UEFI: graphics (GOP) when available, otherwise
//! the firmware's text console. One concrete type, so `Platform::Backend`
//! stays simple.

use super::gop::GopBackend;
use super::text::TextBackend;
use crate::error::{Error, Result};
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

pub enum UefiBackend {
    Text(TextBackend),
    Graphics(GopBackend),
}

impl UefiBackend {
    /// Graphics mode if the firmware offers a usable GOP mode and
    /// `force_text` isn't set; otherwise the text console.
    pub fn new(force_text: bool) -> Result<Self> {
        if !force_text {
            if let Some(gop) = GopBackend::new() {
                return Ok(Self::Graphics(gop));
            }
        }
        Ok(Self::Text(TextBackend::new()?))
    }
}

macro_rules! delegate {
    ($self:ident, $b:ident => $e:expr) => {
        match $self {
            UefiBackend::Text($b) => $e,
            UefiBackend::Graphics($b) => $e,
        }
    };
}

impl Backend for UefiBackend {
    type Error = Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        delegate!(self, b => b.draw(content))
    }

    fn hide_cursor(&mut self) -> Result<()> {
        delegate!(self, b => b.hide_cursor())
    }

    fn show_cursor(&mut self) -> Result<()> {
        delegate!(self, b => b.show_cursor())
    }

    fn get_cursor_position(&mut self) -> Result<Position> {
        delegate!(self, b => b.get_cursor_position())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<()> {
        delegate!(self, b => b.set_cursor_position(position))
    }

    fn clear(&mut self) -> Result<()> {
        delegate!(self, b => b.clear())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<()> {
        delegate!(self, b => b.clear_region(clear_type))
    }

    fn size(&self) -> Result<Size> {
        delegate!(self, b => b.size())
    }

    fn window_size(&mut self) -> Result<WindowSize> {
        delegate!(self, b => b.window_size())
    }

    fn flush(&mut self) -> Result<()> {
        delegate!(self, b => b.flush())
    }
}
