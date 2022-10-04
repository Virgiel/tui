use std::fmt::{Display, Write};
use std::{fmt, io};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use self::buffer::Buffer;

mod buffer;
mod style;
mod terminal;

pub use style::{none, Color, Style};
pub use terminal::Terminal;
pub use crossterm;
pub use unicode_segmentation;
pub use unicode_width;

/// A rectangular area
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Area {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Area {
    /// Remove top line
    fn consume(&mut self) {
        if self.h > 0 {
            self.y += 1;
            self.h -= 1;
        }
    }

    /// Remove bottom line
    fn rconsume(&mut self) {
        if self.h > 0 {
            self.h -= 1
        }
    }
}

/// Measure width of any display
pub fn width(text: impl Display) -> usize {
    struct Measure(usize);

    impl fmt::Write for Measure {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.0 += s.width();
            Ok(())
        }
    }

    let mut measure = Measure(0);
    measure.write_fmt(format_args!("{text}")).unwrap();
    measure.0
}

/// Hidden write to format display text
struct Writer<'a, 'b> {
    line: &'b mut Line<'a>,
    style: Style,
}

impl fmt::Write for Writer<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            // Check visible and fit
            let width = c.width().unwrap_or(0);
            if width == 0 {
                continue;
            } else if width > self.line.remaining {
                break;
            }
            // Put char
            self.line.buf.char_at(self.line.index, c, self.style);
            // Update state
            self.line.index += width;
            self.line.remaining = self.line.remaining.saturating_sub(width);
        }
        Ok(())
    }
}

/// A canvas line
pub struct Line<'a> {
    index: usize,
    remaining: usize,
    buf: &'a mut Buffer,
}

impl<'a> Line<'a> {
    /// Write styled text at the beginning of the line
    pub fn draw(&mut self, text: impl fmt::Display, style: Style) -> &mut Self {
        Writer { line: self, style }
            .write_fmt(format_args!("{text}"))
            .unwrap();
        self
    }

    /// Write a formatted text at the end of the line
    pub fn rdraw(&mut self, text: impl fmt::Display, style: Style) -> &mut Self {
        // We cannot simply write str from right to left as write_ftm is going to split
        // the text into multiple string from left to right, the workaround is to
        // 1: compute the printed length
        let width = width(&text);
        let fit = width.min(self.remaining);
        // 2: create a dummy line containing the required end of the current line
        Line {
            index: self.index + self.remaining - fit,
            remaining: fit,
            buf: self.buf,
        }
        .draw(text, style);
        // 3: apply the lost end part manually
        self.remaining -= fit;
        self
    }

    pub fn width(&self) -> usize {
        self.remaining
    }

    /// Place cursor where we are on the line
    pub fn cursor(&mut self) -> &mut Self {
        self.buf.cursor_pos = Some(self.buf.pos_of(self.index));
        self
    }

    /// Check text fit in remaining space
    pub fn fit(&self, str: impl AsRef<str>) -> bool {
        str.as_ref().width() < self.remaining
    }
}

/// An area of a canvas buffer
pub struct Canvas<'a> {
    area: Area,
    buf: &'a mut Buffer,
}

impl<'a> Canvas<'a> {
    /* ----- Lines ----- */

    /// Get first line
    pub fn top(&mut self) -> Line {
        if self.area.h > 0 {
            let index = self.index_of(0, 0);
            self.area.consume();
            Line {
                index,
                remaining: self.area.w,
                buf: self.buf,
            }
        } else {
            Line {
                index: 0,
                remaining: 0,
                buf: self.buf,
            }
        }
    }

    /// Get last line
    pub fn btm(&mut self) -> Line {
        if self.area.h > 0 {
            let index = self.index_of(0, self.area.h - 1);
            self.area.rconsume();
            Line {
                index,
                remaining: self.area.w,
                buf: self.buf,
            }
        } else {
            Line {
                index: 0,
                remaining: 0,
                buf: self.buf,
            }
        }
    }

    /* ----- Utils ----- */

    pub fn line(&mut self, text: impl Display, style: Style) -> &mut Self {
        self.top().draw(text, style);
        self
    }

    pub fn rline(&mut self, text: impl Display, style: Style) -> &mut Self {
        self.btm().draw(text, style);
        self
    }

    /// Write multilines a the top, wrapping to avoid splitting word
    pub fn wrap(&mut self, string: impl AsRef<str>, style: Style) {
        let mut words = string.as_ref().split_word_bounds().peekable();
        for _ in 0..self.area.h {
            let mut line = self.top();
            loop {
                if let Some(next) = words.peek() {
                    if line.fit(next) {
                        line.draw(words.next().unwrap(), style);
                    } else {
                        break;
                    }
                } else {
                    return;
                }
            }
        }
    }

    /// Get index at pos in canvas part
    fn index_of(&self, x: usize, y: usize) -> usize {
        self.buf.index_of(self.area.x + x, self.area.y + y)
    }

    /* ----- Area ----- */

    /// Covered height
    pub fn height(&self) -> usize {
        self.area.h
    }

    /// Covered width
    pub fn width(&self) -> usize {
        self.area.w
    }

    /// Split canvas
    pub fn split(&self) -> SplitBuilder {
        SplitBuilder {
            area: self.area,
            vertical: false,
            gap: 0,
        }
    }
}

pub struct Split {
    first: Area,
    second: Area,
}

impl Split {
    pub fn first(&self, c: &mut Canvas) {
        c.area = self.first;
    }

    pub fn second(&self, c: &mut Canvas) {
        c.area = self.second;
    }
}

pub struct SplitBuilder {
    area: Area,
    vertical: bool,
    gap: usize,
}

impl SplitBuilder {
    pub fn vertical(mut self, is_vertical: bool) -> Self {
        self.vertical = is_vertical;
        self
    }

    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = gap;
        self
    }

    pub fn apply(self) -> Split {
        if self.vertical {
            let space = self.area.h - self.gap;
            let (first, second) = (space / 2, space / 2 + space % 2);
            Split {
                first: Area {
                    h: first,
                    ..self.area
                },
                second: Area {
                    y: self.area.y + self.gap + first,
                    h: second,
                    ..self.area
                },
            }
        } else {
            let space = self.area.w - self.gap;
            let (first, second) = (space / 2, space / 2 + space % 2);
            Split {
                first: Area {
                    w: first,
                    ..self.area
                },
                second: Area {
                    x: self.area.x + self.gap + first,
                    w: second,
                    ..self.area
                },
            }
        }
    }
}

fn io_err<R>(error: crossterm::Result<R>) -> io::Result<R> {
    error.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}
