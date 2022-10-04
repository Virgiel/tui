use std::io::{self, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::{
        Attribute, Colors, Print, SetAttribute, SetBackgroundColor, SetColors, SetForegroundColor,
    },
    terminal::{
        self, disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

use super::{
    buffer::{Buffer, Cell},
    io_err,
    style::{Color, Modifier},
    Canvas,
};

pub struct Terminal {
    out: io::Stdout,
    draw_buffer: Buffer,
    prev_buffer: Buffer,
    size: (usize, usize),
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(self.out, LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
    }
}

impl Terminal {
    /// Wrapper around Terminal initialization. Each buffer is initialized with a blank string and
    /// default colors for the foreground and the background
    pub fn new(mut stdout: io::Stdout) -> io::Result<Terminal> {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Terminal {
            out: stdout,
            draw_buffer: Buffer::empty(0, 0),
            prev_buffer: Buffer::empty(0, 0),
            size: (0, 0),
        })
    }

    /// Obtains a difference between the previous and the current buffer and passes it to the
    /// current backend for drawing.
    pub fn apply_change(&mut self) -> io::Result<()> {
        let Terminal {
            out, draw_buffer, ..
        } = self;
        let changes = self.prev_buffer.diff(draw_buffer);
        Self::draw_changes(out, changes.into_iter())?;
        std::mem::swap(&mut self.draw_buffer, &mut self.prev_buffer);
        self.draw_buffer.reset();
        Ok(())
    }

    /// Queries the backend for size and resizes if it doesn't match the previous size.
    fn autoresize(&mut self) -> io::Result<()> {
        let (w, h) = io_err(terminal::size().map(|(w, h)| (w as usize, h as usize)))?;
        if (w, h) != self.size {
            self.size = (w, h);
            self.draw_buffer.resize(w, h);
            self.prev_buffer.resize(w, h);
            // Force a full redraw on next frame
            io_err(queue!(self.out, Clear(ClearType::All)))?;
            self.prev_buffer.reset();
        }
        Ok(())
    }

    pub fn suspend_ui(&mut self, f: impl FnOnce()) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(self.out, LeaveAlternateScreen)?;
        f();
        enable_raw_mode()?;
        execute!(self.out, EnterAlternateScreen)?;
        self.prev_buffer.reset();
        Ok(())
    }

    /// Synchronizes terminal size, calls the rendering closure, flushes the current internal state
    /// and prepares for the next draw call.
    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Canvas),
    {
        self.autoresize()?;
        let buf = &mut self.draw_buffer;
        f(&mut buf.canvas());

        let pos = buf.cursor_pos;

        // Draw to stdout
        self.apply_change()?;

        match pos {
            None => io_err(queue!(self.out, Hide))?,
            Some((x, y)) => {
                io_err(queue!(self.out, Show))?;
                io_err(queue!(self.out, MoveTo(x as u16, y as u16)))?;
            }
        }

        // Flush
        self.out.flush()
    }

    fn draw_changes<'a, I>(out: &mut io::Stdout, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut colors = (Color::Reset, Color::Reset);
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<(u16, u16)> = None;

        io_err(queue!(
            out,
            SetForegroundColor(Color::Reset),
            SetBackgroundColor(Color::Reset),
            SetAttribute(Attribute::Reset)
        ))?;

        for (x, y, cell) in content {
            // Move the cursor if the previous location was not (x - 1, y)
            if !matches!(last_pos, Some(p) if x == p.0 + 1 && y == p.1) {
                io_err(queue!(out, MoveTo(x, y)))?;
            }
            last_pos = Some((x, y));
            if cell.modifier != modifier {
                Modifier::diff(out, modifier, cell.modifier)?;
                modifier = cell.modifier;
            }
            let new = (cell.fg, cell.bg);
            match (colors.0 == new.0, colors.1 == new.1) {
                (false, false) => io_err(queue!(out, SetColors(Colors::new(new.0, new.1))))?,
                (false, true) => io_err(queue!(out, SetForegroundColor(new.0)))?,
                (true, false) => io_err(queue!(out, SetBackgroundColor(new.1)))?,
                (true, true) => {}
            }
            colors = new;
            io_err(queue!(out, Print(&cell.char)))?;
        }
        Ok(())
    }
}
