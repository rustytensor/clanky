//! Box-drawing lines, block elements and shades drawn as rectangles instead of font glyphs. A font's line
//! glyphs stop short of the cell when the line height is taller than the font, which leaves gaps between rows;
//! rectangles cut from the cell's own snapped edges always meet their neighbours.

/// A rectangle in device pixels, with the opacity of the foreground colour to fill it with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub alpha: f32,
}

/// A cell's snapped edges in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Weights of the four arms of a box-drawing character: 0 none, 1 light, 2 heavy.
type Arms = [u8; 4]; // left, right, up, down

fn arms(character: char) -> Option<Arms> {
    Some(match character {
        '─' => [1, 1, 0, 0],
        '━' => [2, 2, 0, 0],
        '│' => [0, 0, 1, 1],
        '┃' => [0, 0, 2, 2],
        '┌' => [0, 1, 0, 1],
        '┏' => [0, 2, 0, 2],
        '┐' => [1, 0, 0, 1],
        '┓' => [2, 0, 0, 2],
        '└' => [0, 1, 1, 0],
        '┗' => [0, 2, 2, 0],
        '┘' => [1, 0, 1, 0],
        '┛' => [2, 0, 2, 0],
        '├' => [0, 1, 1, 1],
        '┣' => [0, 2, 2, 2],
        '┤' => [1, 0, 1, 1],
        '┫' => [2, 0, 2, 2],
        '┬' => [1, 1, 0, 1],
        '┳' => [2, 2, 0, 2],
        '┴' => [1, 1, 1, 0],
        '┻' => [2, 2, 2, 0],
        '┼' => [1, 1, 1, 1],
        '╋' => [2, 2, 2, 2],
        '╴' => [1, 0, 0, 0],
        '╵' => [0, 0, 1, 0],
        '╶' => [0, 1, 0, 0],
        '╷' => [0, 0, 0, 1],
        _ => return None,
    })
}

/// Part of a cell as fractions (left, top, right, bottom), with the opacity to fill it with.
type Fraction = (f32, f32, f32, f32, f32);

/// Block elements as fractions of the cell.
fn block(character: char) -> Option<Vec<Fraction>> {
    let eighth = |n: u32| n as f32 / 8.0;
    let code = character as u32;
    Some(match character {
        '█' => vec![(0.0, 0.0, 1.0, 1.0, 1.0)],
        '▀' => vec![(0.0, 0.0, 1.0, 0.5, 1.0)],
        '▔' => vec![(0.0, 0.0, 1.0, eighth(1), 1.0)],
        '▐' => vec![(0.5, 0.0, 1.0, 1.0, 1.0)],
        '▕' => vec![(eighth(7), 0.0, 1.0, 1.0, 1.0)],
        '░' => vec![(0.0, 0.0, 1.0, 1.0, 0.25)],
        '▒' => vec![(0.0, 0.0, 1.0, 1.0, 0.5)],
        '▓' => vec![(0.0, 0.0, 1.0, 1.0, 0.75)],
        // U+2581..U+2587: lower one to seven eighths (▄ is four).
        '\u{2581}'..='\u{2587}' => vec![(0.0, 1.0 - eighth(code - 0x2580), 1.0, 1.0, 1.0)],
        // U+2589..U+258F: left seven down to one eighth (▌ is four).
        '\u{2589}'..='\u{258F}' => vec![(0.0, 0.0, eighth(0x2590 - code), 1.0, 1.0)],
        _ => return quadrants(character),
    })
}

fn quadrants(character: char) -> Option<Vec<Fraction>> {
    // Upper left, upper right, lower left, lower right.
    let filled: [bool; 4] = match character {
        '▘' => [true, false, false, false],
        '▝' => [false, true, false, false],
        '▖' => [false, false, true, false],
        '▗' => [false, false, false, true],
        '▚' => [true, false, false, true],
        '▞' => [false, true, true, false],
        '▙' => [true, false, true, true],
        '▛' => [true, true, true, false],
        '▜' => [true, true, false, true],
        '▟' => [false, true, true, true],
        _ => return None,
    };
    let corners = [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.5, 0.5)];
    Some(
        corners
            .iter()
            .zip(filled)
            .filter(|(_, on)| *on)
            .map(|((x, y), _)| (*x, *y, x + 0.5, y + 0.5, 1.0))
            .collect(),
    )
}

/// Whether the character is drawn by `pieces` rather than as text.
pub fn is_shape(character: char) -> bool {
    arms(character).is_some() || block(character).is_some()
}

/// The rectangles that draw `character` in `cell`. `light` is the light line width in device pixels.
pub fn pieces(character: char, cell: CellBox, light: f32) -> Vec<Piece> {
    let width = cell.right - cell.left;
    let height = cell.bottom - cell.top;
    if let Some(parts) = block(character) {
        // Fractions land on whole device pixels so halves of neighbouring cells never overlap or part.
        let x = |f: f32| cell.left + (width * f).round();
        let y = |f: f32| cell.top + (height * f).round();
        return parts
            .into_iter()
            .map(|(l, t, r, b, alpha)| Piece {
                left: x(l),
                top: y(t),
                right: x(r),
                bottom: y(b),
                alpha,
            })
            .collect();
    }
    let Some([left, right, up, down]) = arms(character) else {
        return Vec::new();
    };
    let thickness = |weight: u8| f32::from(weight) * light;
    // The lines cross at the cell's centre, snapped so the stroke covers whole pixels.
    let centre_x = cell.left + ((width - light) / 2.0).floor();
    let centre_y = cell.top + ((height - light) / 2.0).floor();
    let mut out = Vec::new();
    let horizontal = |from: f32, to: f32, weight: u8| {
        let t = thickness(weight);
        let top = centre_y - ((t - light) / 2.0).floor();
        Piece {
            left: from,
            top,
            right: to,
            bottom: top + t,
            alpha: 1.0,
        }
    };
    let vertical = |from: f32, to: f32, weight: u8| {
        let t = thickness(weight);
        let x = centre_x - ((t - light) / 2.0).floor();
        Piece {
            left: x,
            top: from,
            right: x + t,
            bottom: to,
            alpha: 1.0,
        }
    };
    let reach = |weight: u8| thickness(weight.max(1));
    if left > 0 {
        out.push(horizontal(cell.left, centre_x + reach(up.max(down)), left));
    }
    if right > 0 {
        out.push(horizontal(centre_x, cell.right, right));
    }
    if up > 0 {
        out.push(vertical(cell.top, centre_y + reach(left.max(right)), up));
    }
    if down > 0 {
        out.push(vertical(centre_y, cell.bottom, down));
    }
    out
}

/// Which line under the text a run carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnderlineKind {
    Single,
    Double,
    Curly,
}

/// Where a run's underline may go: the device pixel rows below the baseline, down to the cell's bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnderlineSpace {
    pub left: f32,
    pub right: f32,
    /// The baseline, in device pixels; the first row below it is the first row the line may use.
    pub baseline: f32,
    pub bottom: f32,
    /// Line thickness in device pixels.
    pub thickness: f32,
    /// Where the wave of a curly underline starts its period, and the period's length: one wave per cell, counted
    /// from the grid's left edge, so the waves of neighbouring runs join.
    pub wave_origin: f32,
    pub wave_period: f32,
}

/// The rectangles of an underline, kept inside its own row: a line drawn below the cell would be painted over by
/// the next row's background, and would read as that row's ink.
pub fn underline(kind: UnderlineKind, space: UnderlineSpace) -> Vec<Piece> {
    let t = space.thickness;
    let first = space.baseline.floor() + 1.0;
    let room = (space.bottom - first).max(t);
    let line = |top: f32| Piece {
        left: space.left,
        top,
        right: space.right,
        bottom: top + t,
        alpha: 1.0,
    };
    match kind {
        UnderlineKind::Single => vec![line(first + ((room - t) / 2.0).floor())],
        UnderlineKind::Double => vec![line(space.bottom - 3.0 * t), line(space.bottom - t)],
        UnderlineKind::Curly => curl(space, first, room - t),
    }
}

/// A wave one device pixel wide per column, between `first` and `first + amplitude`; columns at the same height
/// are merged into one rectangle.
fn curl(space: UnderlineSpace, first: f32, amplitude: f32) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    let mut x = space.left;
    while x < space.right {
        let phase = (x + 0.5 - space.wave_origin) / space.wave_period.max(1.0);
        let lift = (amplitude * (1.0 - (phase * std::f32::consts::TAU).cos()) / 2.0).round();
        let top = first + amplitude - lift;
        match out.last_mut() {
            Some(last) if last.top == top && last.right == x => last.right = x + 1.0,
            _ => out.push(Piece {
                left: x,
                top,
                right: x + 1.0,
                bottom: top + space.thickness,
                alpha: 1.0,
            }),
        }
        x += 1.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The pane at scale 1.5: a 29 px row whose baseline lies 24.48 px below its top.
    const SPACE: UnderlineSpace = UnderlineSpace {
        left: 18.0,
        right: 76.0,
        baseline: 187.48,
        bottom: 192.0,
        thickness: 2.0,
        wave_origin: 18.0,
        wave_period: 14.4,
    };

    #[test]
    fn underlines_stay_between_the_baseline_and_the_cell_bottom() {
        for kind in [UnderlineKind::Single, UnderlineKind::Curly] {
            for piece in underline(kind, SPACE) {
                assert!(
                    piece.top >= 188.0 && piece.bottom <= 192.0,
                    "{kind:?} {piece:?}"
                );
            }
        }
        // Four rows below the baseline, a two pixel line: centred, rows 189 and 190.
        assert_eq!(underline(UnderlineKind::Single, SPACE)[0].top, 189.0);
    }

    #[test]
    fn a_curly_underline_covers_every_column_and_waves() {
        let pieces = underline(UnderlineKind::Curly, SPACE);
        let columns: f32 = pieces.iter().map(|p| p.right - p.left).sum();
        assert_eq!(columns, 58.0);
        let tops: Vec<f32> = pieces.iter().map(|p| p.top).collect();
        let highest = tops.iter().copied().fold(f32::MAX, f32::min);
        let lowest = tops.iter().copied().fold(f32::MIN, f32::max);
        assert_eq!((highest, lowest), (188.0, 190.0));
        // At scale 1 (a 19 px row, baseline 16.32 below its top) there are two rows: the wave still moves by one.
        let small = UnderlineSpace {
            baseline: 28.32,
            bottom: 31.0,
            thickness: 1.0,
            wave_period: 9.6,
            ..SPACE
        };
        let tops: Vec<f32> = underline(UnderlineKind::Curly, small)
            .iter()
            .map(|p| p.top)
            .collect();
        assert!(tops.contains(&29.0) && tops.contains(&30.0), "{tops:?}");
    }

    const CELL: CellBox = CellBox {
        left: 10.0,
        top: 20.0,
        right: 20.0,
        bottom: 39.0,
    };

    #[test]
    fn a_horizontal_line_spans_the_whole_cell() {
        let got = pieces('─', CELL, 1.0);
        assert_eq!(got.len(), 2);
        assert_eq!(got.iter().map(|p| p.left).fold(f32::MAX, f32::min), 10.0);
        assert_eq!(got.iter().map(|p| p.right).fold(f32::MIN, f32::max), 20.0);
    }

    #[test]
    fn a_vertical_line_spans_the_whole_row() {
        let got = pieces('│', CELL, 1.0);
        assert_eq!(got.iter().map(|p| p.top).fold(f32::MAX, f32::min), 20.0);
        assert_eq!(got.iter().map(|p| p.bottom).fold(f32::MIN, f32::max), 39.0);
    }

    #[test]
    fn blocks_and_shades_fill_to_the_edges() {
        assert_eq!(
            pieces('█', CELL, 1.0),
            [Piece {
                left: 10.0,
                top: 20.0,
                right: 20.0,
                bottom: 39.0,
                alpha: 1.0
            }]
        );
        // 19 rows: the lower half starts at the rounded middle, 20 + 10.
        assert_eq!(pieces('▄', CELL, 1.0)[0].top, 30.0);
        assert_eq!(pieces('▒', CELL, 1.0)[0].alpha, 0.5);
        assert!(pieces('x', CELL, 1.0).is_empty());
    }
}
