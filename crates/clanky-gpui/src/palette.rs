//! The pane's colours: the config's terminal colours, each unset one taken from Clanky's resolved theme.
//! Cells keep where their colour came from (default, ANSI index or true colour), so a cell's colour is looked
//! up here rather than trusted from the server's built-in palette.

use clanky::config::Config;
use clanky_core::{Cell, ColorSource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
    pub selection: [u8; 3],
    pub ansi: [[u8; 3]; 16],
}

fn rgb(colour: [u8; 4]) -> [u8; 3] {
    [colour[0], colour[1], colour[2]]
}

impl Palette {
    pub fn from_config(config: &Config) -> Self {
        let theme = config.resolved_theme(false);
        let set = &config.terminal_colors;
        let foreground = set.foreground.map_or(theme.terminal_foreground, rgb);
        let background = set.background.map_or(theme.terminal_background, rgb);
        Self {
            foreground,
            background,
            cursor: set.cursor.map_or(foreground, rgb),
            selection: set.selection.map_or(rgb(theme.selection), rgb),
            ansi: set.ansi.map_or(theme.ansi, |ansi| ansi.map(rgb)),
        }
    }

    /// The cell's foreground and background after inverse video: an inverse cell swaps its own two colours.
    pub fn resolve(&self, cell: &Cell) -> ([u8; 3], [u8; 3], bool) {
        let Some(source) = cell.color_source else {
            return (cell.fg, cell.bg, false);
        };
        // For an inverse cell the snapshot swapped the two RGB values but kept each side's source; pair each
        // source with its own value again, resolve both, then swap the resolved colours.
        let (fg_source, bg_source, fg_raw, bg_raw) = if source.inverse {
            (source.fg, source.bg, cell.bg, cell.fg)
        } else {
            (source.fg, source.bg, cell.fg, cell.bg)
        };
        let fg = self.side(fg_source, fg_raw, self.foreground);
        let bg = self.side(bg_source, bg_raw, self.background);
        let default_bg = bg_source == ColorSource::Default;
        if source.inverse {
            (bg, fg, false)
        } else {
            (fg, bg, default_bg)
        }
    }

    fn side(&self, source: ColorSource, raw: [u8; 3], default: [u8; 3]) -> [u8; 3] {
        match source {
            ColorSource::Default => default,
            ColorSource::Indexed(index @ 0..=15) => self.ansi[usize::from(index)],
            ColorSource::Indexed(_) | ColorSource::Truecolor => raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clanky_core::snapshot_from_chunks;

    fn palette() -> Palette {
        let mut ansi = [[0; 3]; 16];
        for (index, colour) in ansi.iter_mut().enumerate() {
            *colour = [index as u8 * 10, 1, 2];
        }
        Palette {
            foreground: [200; 3],
            background: [20; 3],
            cursor: [200; 3],
            selection: [60; 3],
            ansi,
        }
    }

    #[test]
    fn colours_follow_their_source() {
        // Default, ANSI 7, ANSI 196 (kept as the server resolved it), true colour, and inverse of the default.
        let snapshot = snapshot_from_chunks(
            1,
            6,
            &[b"a\x1b[37mb\x1b[0m\x1b[38;5;196mc\x1b[0m\x1b[38;2;1;2;3md\x1b[0m\x1b[7me"],
        )
        .expect("a 1 x 6 terminal is a valid size");
        let got: Vec<([u8; 3], [u8; 3], bool)> = snapshot.cells[..5]
            .iter()
            .map(|cell| palette().resolve(cell))
            .collect();
        assert_eq!(got[0], ([200; 3], [20; 3], true));
        assert_eq!(got[1], ([70, 1, 2], [20; 3], true));
        assert_eq!(got[2].0, snapshot.cells[2].fg);
        assert_eq!(got[3].0, [1, 2, 3]);
        assert_eq!(got[4], ([20; 3], [200; 3], false));
    }
}
