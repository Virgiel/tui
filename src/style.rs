use std::io::{self, Stdout};

use bitflags::bitflags;
use crossterm::{queue, style::SetAttribute};


pub use crossterm::style::{Attribute, Color};

use crate::io_err;

bitflags! {
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
        for removed in IterModifier(from - to) {
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
        for added in IterModifier(to - from) {
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

struct IterModifier(Modifier);

impl Iterator for IterModifier {
    type Item = Modifier;
    fn next(&mut self) -> Option<Modifier> {
        if self.0.is_empty() {
            None
        } else {
            let bits = 1 << self.0.bits().trailing_zeros();
            let r = unsafe { Modifier::from_bits_unchecked(bits) };
            self.0.remove(r);
            Some(r)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub(crate) modifier: Modifier,
}

pub fn none() -> Style {
    Style::default()
}

impl Default for Style {
    fn default() -> Style {
        Style {
            fg: None,
            bg: None,
            modifier: Modifier::empty(),
        }
    }
}

impl Style {
    /// Changes the foreground color
    pub fn fg(mut self, color: Color) -> Style {
        self.fg = Some(color);
        self
    }

    /// Changes the background color
    pub fn bg(mut self, color: Color) -> Style {
        self.bg = Some(color);
        self
    }

    pub fn bold(self) -> Style {
        self.add_modifier(Modifier::BOLD)
    }
    
    pub fn dim(self) -> Style {
        self.add_modifier(Modifier::DIM)
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
