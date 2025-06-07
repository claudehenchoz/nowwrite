use std::io::{self, Result};

pub struct Terminal {
    original_termios: Option<termios::Termios>,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        let original_termios = if let Ok(termios) = termios::Termios::from_fd(0) {
            Some(termios)
        } else {
            None
        };
        Ok(Self { original_termios })
    }

    pub fn enter_raw_mode(&self) -> Result<()> {
        if let Some(ref termios) = self.original_termios {
            let mut raw = termios.clone();
            termios::cfmakeraw(&mut raw);
            termios::tcsetattr(0, termios::TCSANOW, &raw)?;
        }
        Ok(())
    }

    pub fn exit_raw_mode(&self) -> Result<()> {
        if let Some(ref termios) = self.original_termios {
            termios::tcsetattr(0, termios::TCSANOW, termios)?;
        }
        Ok(())
    }

    pub fn enter_alternate_screen(&self) -> Result<()> {
        print!("\x1B[?1049h");
        io::Write::flush(&mut io::stdout())?;
        Ok(())
    }

    pub fn exit_alternate_screen(&self) -> Result<()> {
        print!("\x1B[?1049l");
        io::Write::flush(&mut io::stdout())?;
        Ok(())
    }

    pub fn get_size(&self) -> Result<(usize, usize)> {
        if let Some((width, height)) = terminal_size::terminal_size() {
            Ok((width.0 as usize, height.0 as usize))
        } else {
            Ok((80, 24))
        }
    }

    pub fn set_cursor_position(&self, x: usize, y: usize) -> Result<()> {
        print!("\x1B[{};{}H", y + 1, x + 1);
        Ok(())
    }

    pub fn hide_cursor(&self) -> Result<()> {
        print!("\x1B[?25l");
        io::Write::flush(&mut io::stdout())?;
        Ok(())
    }

    pub fn show_cursor(&self) -> Result<()> {
        print!("\x1B[?25h");
        io::Write::flush(&mut io::stdout())?;
        Ok(())
    }

    pub fn clear_screen(&self) -> Result<()> {
        print!("\x1B[2J\x1B[H");
        io::Write::flush(&mut io::stdout())?;
        Ok(())
    }
}