//! Cells to drawing runs, the way Zed's terminal element lays out its grid: one text run per stretch of
//! same-styled cells on a line, one background rectangle per stretch of same-coloured cells, and box and block
//! characters set apart to be drawn as shapes. Nothing here knows about gpui-ce or about where cells come from.

/// What the layout needs from one cell. Implemented for the session server's (already masked) snapshot cells.
pub trait LayoutCell {
    fn character(&self) -> char;
    fn foreground(&self) -> [u8; 3];
    fn background(&self) -> [u8; 3];
    /// True when the background is the terminal's default, so nothing needs to be painted under the cell.
    fn default_background(&self) -> bool;
    fn style(&self) -> Style;
}

/// The text attributes that change how a run is drawn; one flag per SGR attribute, as the terminal keeps them.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub faint: bool,
    pub italic: bool,
    pub underline: Underline,
    pub strikeout: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
}

/// Text cells drawn together: `text` starts at `column` and covers one cell per character.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub row: usize,
    pub column: usize,
    pub text: String,
    pub colour: [u8; 3],
    pub style: Style,
}

/// A stretch of cells on one row with the same non-default background.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    pub row: usize,
    pub column: usize,
    pub width: usize,
    pub colour: [u8; 3],
}

/// One cell drawn as shapes rather than text (box drawing, block elements, shades).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeCell {
    pub row: usize,
    pub column: usize,
    pub character: char,
    pub colour: [u8; 3],
}

#[derive(Debug, Default, PartialEq)]
pub struct GridLayout {
    pub backgrounds: Vec<Background>,
    pub runs: Vec<TextRun>,
    pub shapes: Vec<ShapeCell>,
}

/// Lays out `rows` x `cols` cells. A cell holding a wide character is followed by a spacer cell (a space), which
/// is skipped; the wide character gets a run of its own so it is placed at its column whatever its advance.
pub fn layout_grid<C: LayoutCell>(cells: &[C], rows: usize, cols: usize) -> GridLayout {
    let mut layout = GridLayout::default();
    for row in 0..rows {
        let line = &cells[row * cols..(row + 1) * cols];
        backgrounds(&mut layout.backgrounds, row, line);
        text_and_shapes(&mut layout, row, line);
    }
    layout
}

fn backgrounds<C: LayoutCell>(out: &mut Vec<Background>, row: usize, line: &[C]) {
    for (column, cell) in line.iter().enumerate() {
        if cell.default_background() {
            continue;
        }
        let colour = cell.background();
        match out.last_mut() {
            Some(last)
                if last.row == row
                    && last.column + last.width == column
                    && last.colour == colour =>
            {
                last.width += 1;
            }
            _ => out.push(Background {
                row,
                column,
                width: 1,
                colour,
            }),
        }
    }
}

fn text_and_shapes<C: LayoutCell>(layout: &mut GridLayout, row: usize, line: &[C]) {
    let mut open: Option<TextRun> = None;
    let mut skip_spacer = false;
    for (column, cell) in line.iter().enumerate() {
        if skip_spacer {
            skip_spacer = false;
            continue;
        }
        let character = cell.character();
        let wide = is_wide(character);
        skip_spacer = wide;
        if crate::glyphs::is_shape(character) {
            layout.runs.extend(open.take());
            layout.shapes.push(ShapeCell {
                row,
                column,
                character,
                colour: cell.foreground(),
            });
            continue;
        }
        let style = cell.style();
        if character == ' ' && style.underline == Underline::None && !style.strikeout {
            layout.runs.extend(open.take());
            continue;
        }
        let colour = cell.foreground();
        // Only plain single-cell characters share a run: their advance is the cell width in a monospace font.
        // Anything else (wide characters, glyphs from a fallback font) is placed at its own column.
        let joins = !wide && character.is_ascii();
        match &mut open {
            Some(run)
                if joins
                    && run.colour == colour
                    && run.style == style
                    && run.column + run.text.len() == column =>
            {
                run.text.push(character);
            }
            _ => {
                layout.runs.extend(open.take());
                let run = TextRun {
                    row,
                    column,
                    text: character.to_string(),
                    colour,
                    style,
                };
                if joins {
                    open = Some(run);
                } else {
                    layout.runs.push(run);
                }
            }
        }
    }
    layout.runs.extend(open);
}

/// Characters the terminal stores in two cells: the width table clanky-vt itself uses.
pub fn is_wide(character: char) -> bool {
    unicode_width::UnicodeWidthChar::width(character) == Some(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Plain {
        character: char,
        fg: [u8; 3],
        bg: Option<[u8; 3]>,
        style: Style,
    }

    impl LayoutCell for Plain {
        fn character(&self) -> char {
            self.character
        }
        fn foreground(&self) -> [u8; 3] {
            self.fg
        }
        fn background(&self) -> [u8; 3] {
            self.bg.unwrap_or([0; 3])
        }
        fn default_background(&self) -> bool {
            self.bg.is_none()
        }
        fn style(&self) -> Style {
            self.style
        }
    }

    fn line(text: &str) -> Vec<Plain> {
        text.chars()
            .map(|character| Plain {
                character,
                fg: [9; 3],
                bg: None,
                style: Style::default(),
            })
            .collect()
    }

    #[test]
    fn words_become_runs_and_spaces_split_them() {
        let cells = line("ab  cd");
        let layout = layout_grid(&cells, 1, 6);
        let runs: Vec<(usize, &str)> = layout
            .runs
            .iter()
            .map(|r| (r.column, r.text.as_str()))
            .collect();
        assert_eq!(runs, [(0, "ab"), (4, "cd")]);
        assert!(layout.backgrounds.is_empty());
    }

    #[test]
    fn wide_characters_stand_alone_and_skip_their_spacer() {
        // The terminal stores 日本 as 日, spacer, 本, spacer; then a blank cell and x.
        let cells = line("日 本  x");
        let layout = layout_grid(&cells, 1, 6);
        let runs: Vec<(usize, &str)> = layout
            .runs
            .iter()
            .map(|r| (r.column, r.text.as_str()))
            .collect();
        assert_eq!(runs, [(0, "日"), (2, "本"), (5, "x")]);
    }

    #[test]
    fn equal_backgrounds_merge_and_box_characters_are_shapes() {
        let mut cells = line("  ─x");
        cells[0].bg = Some([1, 2, 3]);
        cells[1].bg = Some([1, 2, 3]);
        let layout = layout_grid(&cells, 1, 4);
        assert_eq!(
            layout.backgrounds,
            [Background {
                row: 0,
                column: 0,
                width: 2,
                colour: [1, 2, 3]
            }]
        );
        assert_eq!(
            layout.shapes,
            [ShapeCell {
                row: 0,
                column: 2,
                character: '─',
                colour: [9; 3]
            }]
        );
        assert_eq!(layout.runs.len(), 1);
    }

    #[test]
    fn a_style_change_starts_a_new_run() {
        let mut cells = line("abc");
        cells[1].style.bold = true;
        let layout = layout_grid(&cells, 1, 3);
        let runs: Vec<(usize, &str, bool)> = layout
            .runs
            .iter()
            .map(|r| (r.column, r.text.as_str(), r.style.bold))
            .collect();
        assert_eq!(runs, [(0, "a", false), (1, "b", true), (2, "c", false)]);
    }
}
