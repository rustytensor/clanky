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

#[cfg(test)]
mod tests {
    use super::*;

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
