use std::io::{self, Stdout};

use bitflags::bitflags;
use crossterm::{queue, style::SetAttribute};

pub use crossterm::style::{Attribute, Color};

use crate::io_err;

bitflags! {
    #[derive(Clone, Debug, Copy, PartialEq, Eq)]
    pub(crate) struct Modifier: u8 {
        const BOLD              = 0b0000_0001;
        const DIM               = 0b0000_0010;
        const ITALIC            = 0b0000_0100;
        const UNDERLINED        = 0b0000_1000;
        const REVERSED          = 0b0001_0000;
        const CROSSED_OUT       = 0b0010_0000;
    }
}

impl Modifier {
    pub fn diff(w: &mut Stdout, from: Modifier, to: Modifier) -> io::Result<()> {
        for removed in (from - to).iter() {
            match removed {
                Modifier::REVERSED => io_err(queue!(w, SetAttribute(Attribute::NoReverse)))?,
                Modifier::BOLD => io_err(queue!(w, SetAttribute(Attribute::NormalIntensity)))?,
                Modifier::ITALIC => io_err(queue!(w, SetAttribute(Attribute::NoItalic)))?,
                Modifier::UNDERLINED => io_err(queue!(w, SetAttribute(Attribute::NoUnderline)))?,
                Modifier::DIM => io_err(queue!(w, SetAttribute(Attribute::NormalIntensity)))?,
                Modifier::CROSSED_OUT => io_err(queue!(w, SetAttribute(Attribute::NotCrossedOut)))?,
                _ => unreachable!("Unknown modifier flag"),
            }
        }
        for added in (to - from).iter() {
            match added {
                Modifier::REVERSED => io_err(queue!(w, SetAttribute(Attribute::Reverse)))?,
                Modifier::BOLD => io_err(queue!(w, SetAttribute(Attribute::Bold)))?,
                Modifier::ITALIC => io_err(queue!(w, SetAttribute(Attribute::Italic)))?,
                Modifier::UNDERLINED => io_err(queue!(w, SetAttribute(Attribute::Underlined)))?,
                Modifier::DIM => io_err(queue!(w, SetAttribute(Attribute::Dim)))?,
                Modifier::CROSSED_OUT => io_err(queue!(w, SetAttribute(Attribute::CrossedOut)))?,
                _ => unreachable!("Unknown modifier flag"),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub(crate) modifier: Modifier,
}

pub const fn none() -> Style {
    Style {
        fg: None,
        bg: None,
        modifier: Modifier::empty(),
    }
}

impl Default for Style {
    fn default() -> Style {
        none()
    }
}

impl Style {
    /// Changes the foreground color
    pub const fn fg(mut self, color: Color) -> Style {
        self.fg = Some(color);
        self
    }

    /// Changes the background color
    pub const fn bg(mut self, color: Color) -> Style {
        self.bg = Some(color);
        self
    }

    pub fn bold(self) -> Style {
        self.add_modifier(Modifier::BOLD)
    }

    pub fn dim(self) -> Style {
        self.add_modifier(Modifier::DIM)
    }
    
    pub fn italic(self) -> Style {
        self.add_modifier(Modifier::ITALIC)
    }

    pub fn underline(self) -> Style {
        self.add_modifier(Modifier::UNDERLINED)
    }

    pub fn reversed(self) -> Style {
        self.add_modifier(Modifier::REVERSED)
    }

    pub fn croosed_out(self) -> Style {
        self.add_modifier(Modifier::CROSSED_OUT)
    }

    pub fn clear_emphasis(self) -> Style {
        self.remove_modifier(Modifier::all())
    }

    /// Changes the text emphasis
    fn add_modifier(mut self, modifier: Modifier) -> Style {
        self.modifier.insert(modifier);
        self
    }

    /// Changes the text emphasis
    fn remove_modifier(mut self, modifier: Modifier) -> Style {
        self.modifier.remove(modifier);
        self
    }
}
