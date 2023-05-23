use std::fmt::{Display, Write};
use std::{fmt, io};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use self::buffer::Buffer;

mod buffer;
mod style;
mod terminal;

pub use crossterm;
pub use style::{none, Color, Style};
pub use terminal::Terminal;
pub use unicode_segmentation;
pub use unicode_width;

/// A rectangular area
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Area {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Area {
    /// Consume top lines
    fn top(&mut self, h: usize) -> Area {
        let tmp = Area {
            h: h.min(self.h),
            ..*self
        };

        self.y += tmp.h;
        self.h -= tmp.h;
        tmp
    }

    /// Consume btm lines
    fn btm(&mut self, h: usize) -> Area {
        let tmp = Area {
            h: h.min(self.h),
            y: self.y + self.h - h.min(self.h),
            ..*self
        };

        self.h -= tmp.h;
        tmp
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
    pub fn new(c: &'a mut Canvas, area: Area) -> Self {
        assert!(area.h <= 1);
        if area.h > 0 {
            Line {
                index: c.buf.index_of(area.x, area.y),
                remaining: area.w,
                buf: c.buf,
            }
        } else {
            Line {
                index: 0,
                remaining: 0,
                buf: c.buf,
            }
        }
    }

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
    /// Line from reserved area
    pub fn consume(&mut self, area: Area) -> &mut Self {
        self.area = area;
        self
    }

    /* ----- Lines ----- */

    /// Get first line
    pub fn top(&mut self) -> Line {
        let area = self.area.top(1);
        Line::new(self, area)
    }

    /// Get last line
    pub fn btm(&mut self) -> Line {
        let area = self.area.btm(1);
        Line::new(self, area)
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
    pub fn split(&mut self) -> SplitBuilder {
        SplitBuilder {
            area: std::mem::take(&mut self.area),
            vertical: false,
            gap: 0,
        }
    }

    /// Reserve top lines
    pub fn reserve_top(&mut self, n: usize) -> Area {
        self.area.top(n)
    }

    /// Reserve btm lines
    pub fn reserve_btm(&mut self, n: usize) -> Area {
        self.area.btm(n)
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

    pub fn apply(self) -> (Area, Area) {
        if self.vertical {
            let space = self.area.h - self.gap;
            let (first, second) = (space / 2, space / 2 + space % 2);
            (
                Area {
                    h: first,
                    ..self.area
                },
                Area {
                    y: self.area.y + self.gap + first,
                    h: second,
                    ..self.area
                },
            )
        } else {
            let space = self.area.w - self.gap;
            let (first, second) = (space / 2, space / 2 + space % 2);
            (
                Area {
                    w: first,
                    ..self.area
                },
                Area {
                    x: self.area.x + self.gap + first,
                    w: second,
                    ..self.area
                },
            )
        }
    }
}

fn io_err<R>(error: crossterm::Result<R>) -> io::Result<R> {
    error.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}
