//! The terminal grid as a gpui-ce element, drawn Zed's way: everything is rebuilt in each prepaint from the
//! latest snapshot (merged background rectangles, one shaped line per run, box and block characters as
//! rectangles, then selection and cursor), and every cell edge sits on a whole device pixel.

use std::{cell::Cell as Shared, rc::Rc, sync::Arc};

use clanky_core::{
    CURSOR_BAR, CURSOR_UNDERLINE, Cell, STYLE_BOLD, STYLE_DOUBLE_UNDERLINE, STYLE_FAINT,
    STYLE_STRIKEOUT, STYLE_TERMINAL_ITALIC, STYLE_UNDERCURL, STYLE_UNDERLINE, TerminalSnapshot,
};
use gpui::{
    App, Bounds, ContentMask, Context, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Font, FontStyle, FontWeight, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, SharedString, StrikethroughStyle,
    Style, TextAlign, TextRun, Window, point, px, relative, rgb_to_hsla, rgba, size,
};

use crate::{
    cells::{self, LayoutCell, Underline},
    glyphs::{self, CellBox},
    palette::Palette,
    pane::TerminalPane,
};

/// Space between the window edge and the grid on every side, in logical pixels.
pub const PANE_PADDING: f32 = 12.0;

/// Where the grid sits, in device pixels; kept by the pane for turning pointer positions into cells.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub scale: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub cell_width: f32,
    pub line_height: f32,
    /// The regular font's ascent: every row's baseline lies this far below the row's top, to the nearest
    /// device pixel, where gpui-ce's own line layout puts it.
    pub ascent: f32,
    pub rows: usize,
    pub cols: usize,
    /// The pane's whole width and height, padding included.
    pub pane_width: f32,
    pub pane_height: f32,
}

impl Metrics {
    /// The cell under a point in logical pixels, and whether it is over the cell's right half.
    pub fn cell_at(&self, x: Pixels, y: Pixels) -> (usize, usize, bool) {
        let dx = (f32::from(x) * self.scale - self.origin_x).max(0.0);
        let dy = (f32::from(y) * self.scale - self.origin_y).max(0.0);
        let col = (dx / self.cell_width) as usize;
        let row = (dy / self.line_height) as usize;
        let right = dx - col as f32 * self.cell_width > self.cell_width / 2.0;
        (
            row.min(self.rows.saturating_sub(1)),
            col.min(self.cols.saturating_sub(1)),
            right,
        )
    }

    /// The first device pixel of a column: the first pixel whose centre lies inside the cell, so neighbouring
    /// cells share their edge and a cell covers exactly the pixels it would cover on paper.
    fn x(&self, col: usize) -> f32 {
        (self.origin_x + col as f32 * self.cell_width - 0.5).ceil()
    }

    fn logical(&self, device: f32) -> Pixels {
        px(device / self.scale)
    }
}

/// The font the pane draws with, read from the config.
#[derive(Clone)]
pub struct TextSettings {
    pub family: SharedString,
    /// Logical pixels.
    pub size: f32,
    /// Line height as a multiple of the font size.
    pub line_height: f32,
}

/// What owns a grid: it takes the keyboard's text through the input handler, and hears the grid's size in cells
/// each time the grid is laid out.
pub trait GridOwner: EntityInputHandler {
    fn fit(&mut self, rows: usize, cols: usize, cx: &mut Context<Self>);
}

pub struct GridElement<V: GridOwner = TerminalPane> {
    pub pane: Entity<V>,
    pub snapshot: Option<Arc<TerminalSnapshot>>,
    pub palette: Palette,
    pub text: TextSettings,
    pub selection: Vec<(usize, usize, usize)>,
    /// The find bar's matches: row, first and end column, and fill.
    pub found: Vec<(usize, usize, usize, [u8; 3])>,
    pub focus: FocusHandle,
    pub metrics: Rc<Shared<Metrics>>,
    /// False while another place (the command input) has the caret.
    pub show_cursor: bool,
    /// Each row's top in device pixels below the grid's origin, when rows have space between command blocks;
    /// None for the plain grid.
    pub tops: Option<Rc<[f32]>>,
}

struct SnapshotCell<'a> {
    cell: &'a Cell,
    palette: &'a Palette,
}

impl LayoutCell for SnapshotCell<'_> {
    fn character(&self) -> char {
        self.cell.ch
    }
    fn foreground(&self) -> [u8; 3] {
        let (fg, bg, _) = self.palette.resolve(self.cell);
        // Faint text (SGR 2) is drawn halfway between its colour and the background.
        if self.cell.style & STYLE_FAINT != 0 {
            std::array::from_fn(|i| fg[i] / 2 + bg[i] / 2)
        } else {
            fg
        }
    }
    fn background(&self) -> [u8; 3] {
        self.palette.resolve(self.cell).1
    }
    fn default_background(&self) -> bool {
        self.palette.resolve(self.cell).2
    }
    fn style(&self) -> cells::Style {
        let style = self.cell.style;
        let underline = if style & STYLE_UNDERCURL != 0 {
            Underline::Curly
        } else if style & STYLE_DOUBLE_UNDERLINE != 0 {
            Underline::Double
        } else if style & STYLE_UNDERLINE != 0 {
            Underline::Single
        } else {
            Underline::None
        };
        cells::Style {
            bold: style & STYLE_BOLD != 0,
            faint: style & STYLE_FAINT != 0,
            italic: style & STYLE_TERMINAL_ITALIC != 0,
            underline,
            strikeout: style & STYLE_STRIKEOUT != 0,
        }
    }
}

pub fn colour(value: [u8; 3]) -> Hsla {
    rgb_to_hsla(rgba(
        u32::from(value[0]) << 24 | u32::from(value[1]) << 16 | u32::from(value[2]) << 8 | 0xff,
    ))
}

fn filled(bounds: Bounds<Pixels>, colour: Hsla) -> PaintQuad {
    PaintQuad::new(bounds).background(colour)
}

fn colour_alpha(value: [u8; 3], alpha: f32) -> Hsla {
    let mut c = colour(value);
    c.alpha = alpha;
    c
}

impl<V: GridOwner> IntoElement for GridElement<V> {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl<V: GridOwner> Element for GridElement<V> {
    type RequestLayoutState = ();
    type PrepaintState = Option<cells::GridLayout>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let metrics = self.measure(bounds, window);
        self.metrics.set(metrics);
        let (rows, cols) = (metrics.rows, metrics.cols);
        self.pane.update(cx, |pane, cx| pane.fit(rows, cols, cx));
        let snapshot = self.snapshot.as_ref()?;
        let shown: Vec<SnapshotCell> = snapshot
            .cells
            .iter()
            .map(|cell| SnapshotCell {
                cell,
                palette: &self.palette,
            })
            .collect();
        Some(cells::layout_grid(&shown, snapshot.rows, snapshot.cols))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let m = self.metrics.get();
        // The pane around the grid paints the terminal background, with its rounded corners.
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, self.pane.clone()),
            cx,
        );
        let Some(layout) = layout.as_ref() else {
            return;
        };
        // Rows lifted above the first row (to keep the newest one in view) stay under the pane's top padding.
        let clip = Bounds::new(
            point(bounds.origin.x, m.logical(m.origin_y)),
            size(
                bounds.size.width,
                m.logical(
                    (m.pane_height - m.origin_y + (f32::from(bounds.origin.y) * m.scale).floor())
                        .max(0.0),
                ),
            ),
        );
        window.with_content_mask(
            Some(ContentMask {
                bounds: clip,
                fade_out: gpui::Edges::default(),
            }),
            |window| {
                self.paint_grid(layout, m, window, cx);
            },
        );
    }
}

impl<V: GridOwner> GridElement<V> {
    /// A row's top in device pixels.
    fn row_top(&self, m: Metrics, row: usize) -> f32 {
        m.origin_y
            + self
                .tops
                .as_ref()
                .and_then(|tops| tops.get(row).copied())
                .unwrap_or(row as f32 * m.line_height)
    }

    fn cell_box(&self, m: Metrics, row: usize, col: usize, width: usize) -> CellBox {
        let top = self.row_top(m, row);
        CellBox {
            left: m.x(col),
            top,
            right: m.x(col + width),
            bottom: top + m.line_height,
        }
    }

    fn paint_grid(
        &self,
        layout: &cells::GridLayout,
        m: Metrics,
        window: &mut Window,
        cx: &mut App,
    ) {
        let device = |b: CellBox| {
            Bounds::new(
                point(m.logical(b.left), m.logical(b.top)),
                size(m.logical(b.right - b.left), m.logical(b.bottom - b.top)),
            )
        };
        for bg in &layout.backgrounds {
            window.paint_quad(filled(
                device(self.cell_box(m, bg.row, bg.column, bg.width)),
                colour(bg.colour),
            ));
        }
        for &(row, start, end, fill) in &self.found {
            window.paint_quad(filled(
                device(self.cell_box(m, row, start, end - start)),
                colour(fill),
            ));
        }
        for &(row, start, end) in &self.selection {
            window.paint_quad(filled(
                device(self.cell_box(m, row, start, end - start)),
                colour(self.palette.selection),
            ));
        }
        let light = (m.scale).round().max(1.0);
        for shape in &layout.shapes {
            for piece in glyphs::pieces(
                shape.character,
                self.cell_box(m, shape.row, shape.column, 1),
                light,
            ) {
                let b = CellBox {
                    left: piece.left,
                    top: piece.top,
                    right: piece.right,
                    bottom: piece.bottom,
                };
                window.paint_quad(filled(device(b), colour_alpha(shape.colour, piece.alpha)));
            }
        }
        for run in &layout.runs {
            self.paint_run(run, m, window, cx);
        }
        if self.show_cursor {
            self.paint_cursor(m, window, cx);
        }
    }

    fn measure(&self, bounds: Bounds<Pixels>, window: &Window) -> Metrics {
        let scale = window.scale_factor();
        let font = self.font(false, false);
        let font_id = window.text_system().resolve_font(&font);
        // Cell width: the advance of 'm', not rounded, as Zed and the gate both define it.
        let advance = window
            .text_system()
            .advance(font_id, px(self.text.size), 'm')
            .map_or(self.text.size * 0.6, |a| f32::from(a.width));
        let cell_width = advance * scale;
        let line_height = (self.text.size * self.text.line_height * scale)
            .round()
            .max(1.0);
        let ascent = f32::from(window.text_system().ascent(font_id, px(self.text.size))) * scale;
        let pad = (PANE_PADDING * scale).floor();
        let origin_x = (f32::from(bounds.origin.x) * scale).floor() + pad;
        let origin_y = (f32::from(bounds.origin.y) * scale).floor() + pad;
        let width = f32::from(bounds.size.width) * scale - 2.0 * pad;
        let height = f32::from(bounds.size.height) * scale - 2.0 * pad;
        let cols = ((width / cell_width).floor() as usize).max(2);
        let rows = ((height / line_height).floor() as usize).max(1);
        Metrics {
            scale,
            origin_x,
            origin_y,
            cell_width,
            line_height,
            ascent,
            rows,
            cols,
            pane_width: f32::from(bounds.size.width) * scale,
            pane_height: f32::from(bounds.size.height) * scale,
        }
    }

    fn font(&self, bold: bool, italic: bool) -> Font {
        let mut font = gpui::font(self.text.family.clone());
        font.weight = if bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        font.style = if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };
        // Programming ligatures would merge cells.
        font.features = gpui::FontFeatures::disable_ligatures();
        font
    }

    fn paint_run(&self, run: &cells::TextRun, m: Metrics, window: &mut Window, cx: &mut App) {
        let style = run.style;
        let thickness = m.scale.round().max(1.0);
        let text_run = TextRun {
            len: run.text.len(),
            font: self.font(style.bold, style.italic),
            color: colour(run.colour),
            background_color: None,
            underline: None,
            strikethrough: style.strikeout.then_some(StrikethroughStyle {
                thickness: m.logical(thickness),
                color: None,
            }),
            letter_spacing: None,
        };
        let line = window.text_system().shape_line(
            run.text.clone().into(),
            px(self.text.size),
            &[text_run],
        );
        // Text starts at the cell's exact edge (not snapped): glyphs are drawn at sub-pixel positions, and the
        // gate places each glyph at column x cell width.
        let row_top = self.row_top(m, run.row);
        let origin = point(
            m.logical(m.origin_x + run.column as f32 * m.cell_width),
            m.logical(row_top),
        );
        // A paint error (a glyph the rasteriser refused) leaves that run blank; the next frame tries again.
        let _ = line.paint(
            origin,
            m.logical(m.line_height),
            TextAlign::Start,
            None,
            window,
            cx,
        );
        let kind = match style.underline {
            Underline::None => return,
            Underline::Single => glyphs::UnderlineKind::Single,
            Underline::Double => glyphs::UnderlineKind::Double,
            Underline::Curly => glyphs::UnderlineKind::Curly,
        };
        let space = glyphs::UnderlineSpace {
            left: m.x(run.column),
            right: m.x(run.column
                + run
                    .text
                    .chars()
                    .map(|c| if cells::is_wide(c) { 2 } else { 1 })
                    .sum::<usize>()),
            baseline: row_top + m.ascent,
            bottom: row_top + m.line_height,
            thickness,
            wave_origin: m.origin_x,
            wave_period: m.cell_width,
        };
        for piece in glyphs::underline(kind, space) {
            window.paint_quad(filled(
                Bounds::new(
                    point(m.logical(piece.left), m.logical(piece.top)),
                    size(
                        m.logical(piece.right - piece.left),
                        m.logical(piece.bottom - piece.top),
                    ),
                ),
                colour(run.colour),
            ));
        }
    }

    fn paint_cursor(&self, m: Metrics, window: &mut Window, cx: &mut App) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some((row, col)) = snapshot.cursor else {
            return;
        };
        let cell = self.cell_box(m, row, col, 1);
        let focused = self.focus.is_focused(window);
        let colour_cursor = colour(self.palette.cursor);
        let stroke = m.scale.round().max(1.0);
        // Bar and underline cursors: 1.5 logical pixels, at least one device pixel (2 px at scales 1 and 1.5).
        let strip = (1.5 * m.scale).round().max(1.0);
        let rect = |l: f32, t: f32, r: f32, b: f32| {
            Bounds::new(
                point(m.logical(l), m.logical(t)),
                size(m.logical(r - l), m.logical(b - t)),
            )
        };
        match snapshot.cursor_shape {
            CURSOR_BAR => window.paint_quad(filled(
                rect(cell.left, cell.top, cell.left + strip, cell.bottom),
                colour_cursor,
            )),
            CURSOR_UNDERLINE => window.paint_quad(filled(
                rect(cell.left, cell.bottom - strip, cell.right, cell.bottom),
                colour_cursor,
            )),
            _ if !focused => {
                // An unfocused window shows a hollow block, as VTE does.
                for (l, t, r, b) in [
                    (cell.left, cell.top, cell.right, cell.top + stroke),
                    (cell.left, cell.bottom - stroke, cell.right, cell.bottom),
                    (cell.left, cell.top, cell.left + stroke, cell.bottom),
                    (cell.right - stroke, cell.top, cell.right, cell.bottom),
                ] {
                    window.paint_quad(filled(rect(l, t, r, b), colour_cursor));
                }
            }
            _ => {
                window.paint_quad(filled(
                    rect(cell.left, cell.top, cell.right, cell.bottom),
                    colour_cursor,
                ));
                // The character under a block cursor is drawn again in the background colour.
                let under = &snapshot.cells[row * snapshot.cols + col];
                if under.ch != ' ' && !glyphs::is_shape(under.ch) {
                    let run = cells::TextRun {
                        row,
                        column: col,
                        text: under.ch.to_string(),
                        colour: self.palette.background,
                        style: cells::Style::default(),
                    };
                    self.paint_run(&run, m, window, cx);
                }
            }
        }
    }
}
