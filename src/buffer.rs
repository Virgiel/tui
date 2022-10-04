use crossterm::style::Color;
use unicode_width::UnicodeWidthChar;

use super::{
    style::{Modifier, Style},
    Area, Canvas,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Buffer {
    pub nb_row: usize,
    pub nb_col: usize,
    pub content: Vec<Cell>,
    pub cursor_pos: Option<(usize, usize)>,
}

impl Buffer {
    /// Returns a Buffer with all cells set to the default one
    pub fn empty(nb_col: usize, nb_row: usize) -> Buffer {
        let cell: Cell = Default::default();
        Buffer::filled(nb_col, nb_row, &cell)
    }

    /// Returns a Buffer with all cells initialized with the attributes of the given Cell
    fn filled(nb_col: usize, nb_row: usize, cell: &Cell) -> Buffer {
        let size = nb_col * nb_row;
        let mut content = Vec::with_capacity(size);
        for _ in 0..size {
            content.push(cell.clone());
        }
        Buffer {
            nb_col,
            nb_row,
            content,
            cursor_pos: None,
        }
    }

    pub fn canvas(&mut self) -> Canvas<'_> {
        Canvas {
            area: Area {
                x: 0,
                y: 0,
                w: self.nb_col,
                h: self.nb_row,
            },
            buf: self,
        }
    }

    pub fn index_of(&self, x: usize, y: usize) -> usize {
        debug_assert!(
            x < self.nb_col && y < self.nb_row,
            "Trying to access position outside the buffer: x={}, y={}, {}x{}",
            x,
            y,
            self.nb_col,
            self.nb_row
        );
        y * self.nb_col + x
    }

    pub fn pos_of(&self, i: usize) -> (usize, usize) {
        debug_assert!(
            i < self.content.len(),
            "Trying to get the coords of a cell outside the buffer: i={} len={}",
            i,
            self.content.len()
        );
        (i % self.nb_col, i / self.nb_col)
    }

    pub fn char_at(&mut self, index: usize, c: char, style: Style) {
        self.content[index].set_char(c).set_style(style);
    }

    /// Resize the buffer so that the mapped area matches the given area and that the buffer
    /// length is equal to area.width * area.height
    pub fn resize(&mut self, nb_col: usize, nb_row: usize) {
        self.content.resize(nb_col * nb_row, Default::default());
        self.nb_col = nb_col;
        self.nb_row = nb_row;
    }

    /// Reset all cells in the buffer
    pub fn reset(&mut self) {
        self.cursor_pos.take();
        for c in &mut self.content {
            c.reset();
        }
    }

    /// Builds a minimal sequence of coordinates and Cells necessary to update the UI from
    /// self to other.
    pub fn diff<'a>(&self, other: &'a Buffer) -> Vec<(u16, u16, &'a Cell)> {
        let previous_buffer = &self.content;
        let next_buffer = &other.content;
        let width = self.nb_col;

        let mut updates: Vec<(u16, u16, &Cell)> = vec![];
        let mut skip: bool = false;
        for (i, (current, previous)) in next_buffer.iter().zip(previous_buffer.iter()).enumerate() {
            if (current != previous) && !skip {
                let x = i as u16 % width as u16;
                let y = i as u16 / width as u16;
                updates.push((x, y, &next_buffer[i]));
            }

            skip = current.char.width().unwrap_or(0) > 1;
        }
        updates
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cell {
    pub char: char,
    pub fg: Color,
    pub bg: Color,
    pub modifier: Modifier,
}

impl Cell {
    pub fn set_char(&mut self, ch: char) -> &mut Cell {
        self.char = ch;
        self
    }

    pub fn set_style(&mut self, style: Style) -> &mut Cell {
        if let Some(c) = style.fg {
            self.fg = c;
        }
        if let Some(c) = style.bg {
            self.bg = c;
        }
        self.modifier = style.modifier;
        self
    }

    pub fn reset(&mut self) {
        self.char = ' ';
        self.fg = Color::Reset;
        self.bg = Color::Reset;
        self.modifier = Modifier::empty();
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            char: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
            modifier: Modifier::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthStr;

    use crate::style::none;

    use super::*;

    fn cell(s: char) -> Cell {
        let mut cell = Cell::default();
        cell.set_char(s);
        cell
    }

    fn buf_lines(lines: &[&str]) -> Buffer {
        let height = lines.len();
        let width = lines.iter().map(|i| i.width()).max().unwrap_or_default();
        let mut buffer = Buffer::empty(width, height);
        let mut c = buffer.canvas();
        for line in lines.iter() {
            c.line(line, none());
        }
        buffer
    }

    #[test]
    fn it_translates_to_and_from_coordinates() {
        let buf = Buffer::empty(50, 80);

        // First cell is at the upper left corner.
        assert_eq!(buf.pos_of(0), (0, 0));
        assert_eq!(buf.index_of(0, 0), 0);

        // Last cell is in the lower right.
        assert_eq!(buf.pos_of(buf.content.len() - 1), (49, 79));
        assert_eq!(buf.index_of(49, 79), buf.content.len() - 1);
    }

    #[test]
    #[should_panic(expected = "outside the buffer")]
    fn pos_of_panics_on_out_of_bounds() {
        let buf = Buffer::empty(10, 10);

        // There are a total of 100 cells; zero-indexed means that 100 would be the 101st cell.
        buf.pos_of(100);
    }

    #[test]
    #[should_panic(expected = "outside the buffer")]
    fn index_of_panics_on_out_of_bounds() {
        let buf = Buffer::empty(10, 10);

        // width is 10; zero-indexed means that 10 would be the 11th cell.
        buf.index_of(10, 0);
    }

    #[test]
    fn buffer_set_string() {
        let mut buffer = Buffer::empty(5, 1);

        // Zero-width
        buffer.canvas().line("", none());
        assert_eq!(buffer, buf_lines(&["     "]));

        buffer.canvas().line("aaa", none());
        assert_eq!(buffer, buf_lines(&["aaa  "]));

        // Width limit:
        buffer.canvas().line("bbbb", none());
        assert_eq!(buffer, buf_lines(&["bbbb "]));

        buffer.canvas().line("12345", none());
        assert_eq!(buffer, buf_lines(&["12345"]));

        // Width truncation:
        buffer.canvas().line("123456", none());
        assert_eq!(buffer, buf_lines(&["12345"]));
    }

    #[test]
    fn buffer_set_string_zero_width() {
        let mut buffer = Buffer::empty(1, 1);

        // Leading grapheme with zero width
        let s = "\u{1}a";
        buffer.canvas().line(s, none());
        assert_eq!(buffer, buf_lines(&["a"]));

        // Trailing grapheme with zero with
        let s = "a\u{1}";
        buffer.canvas().line(s, none());
        assert_eq!(buffer, buf_lines(&["a"]));
    }

    #[test]
    fn buffer_set_string_double_width() {
        let mut buffer = Buffer::empty(5, 1);
        buffer.canvas().line("コン", none());
        assert_eq!(buffer, buf_lines(&["コン "]));

        // Only 1 space left.
        buffer.canvas().line("コンピ", none());
        assert_eq!(buffer, buf_lines(&["コン "]));
    }

    #[test]
    fn buffer_with_lines() {
        let buffer = buf_lines(&["┌────────┐", "│コンピュ│", "│ーa 上で│", "└────────┘"]);
        assert_eq!(buffer.nb_col, 10);
        assert_eq!(buffer.nb_row, 4);
    }

    #[test]
    fn buffer_diffing_empty_empty() {
        let prev = Buffer::empty(40, 40);
        let next = Buffer::empty(40, 40);
        let diff = prev.diff(&next);
        assert_eq!(diff, vec![]);
    }

    #[test]
    fn buffer_diffing_empty_filled() {
        let prev = Buffer::empty(40, 40);
        let next = Buffer::filled(40, 40, &cell('a'));
        let diff = prev.diff(&next);
        assert_eq!(diff.len(), 40 * 40);
    }

    #[test]
    fn buffer_diffing_filled_filled() {
        let prev = Buffer::filled(40, 40, &cell('a'));
        let next = Buffer::filled(40, 40, &cell('a'));
        let diff = prev.diff(&next);
        assert_eq!(diff, vec![]);
    }

    #[test]
    fn buffer_diffing_single_width() {
        let prev = buf_lines(&[
            "          ",
            "┌Title─┐  ",
            "│      │  ",
            "│      │  ",
            "└──────┘  ",
        ]);
        let next = buf_lines(&[
            "          ",
            "┌TITLE─┐  ",
            "│      │  ",
            "│      │  ",
            "└──────┘  ",
        ]);
        let diff = prev.diff(&next);
        assert_eq!(
            diff,
            vec![
                (2, 1, &cell('I')),
                (3, 1, &cell('T')),
                (4, 1, &cell('L')),
                (5, 1, &cell('E')),
            ]
        );
    }

    #[test]
    fn buffer_diffing_multi_width() {
        let prev = buf_lines(&["┌Title─┐  ", "└──────┘  "]);
        let next = buf_lines(&["┌称号──┐  ", "└──────┘  "]);
        let diff = prev.diff(&next);
        assert_eq!(
            diff,
            vec![
                (1, 0, &cell('称')),
                // Skipped "i"
                (3, 0, &cell('号')),
                // Skipped "l"
                (5, 0, &cell('─')),
            ]
        );
    }

    #[test]
    fn buffer_diffing_multi_width_offset() {
        let prev = buf_lines(&["┌称号──┐"]);
        let next = buf_lines(&["┌─称号─┐"]);

        let diff = prev.diff(&next);
        assert_eq!(
            diff,
            vec![(1, 0, &cell('─')), (2, 0, &cell('称')), (4, 0, &cell('号')),]
        );
    }
}
