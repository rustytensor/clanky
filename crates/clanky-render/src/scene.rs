//! Pixel-space scene contract. Colours supplied by callers are straight RGBA;
//! rendered targets and readback pixels use premultiplied RGBA.
use crate::{Renderer, Underline};
use clanky_core::TerminalSnapshot;

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub clear: [u8; 4],
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug)]
pub enum Layer {
    Shape(Shape),
    Grid(GridLayer),
    Text(TextRun),
}

#[derive(Clone, Debug)]
pub struct Shape {
    /// x, y, width, height in physical pixels.
    pub rect: [f32; 4],
    pub radius: f32,
    pub fill: Fill,
    /// Width in physical pixels and straight RGBA colour.
    pub border: Option<(f32, [u8; 4])>,
    pub shadow: Option<Shadow>,
}

#[derive(Clone, Copy, Debug)]
pub enum Fill {
    Solid([u8; 4]),
    /// Top and bottom straight RGBA colours.
    Vertical([u8; 4], [u8; 4]),
}

#[derive(Clone, Copy, Debug)]
pub struct Shadow {
    pub offset: [f32; 2],
    /// Falloff radius in physical pixels; zero produces a hard edge.
    pub blur: f32,
    pub spread: f32,
    pub color: [u8; 4],
}

#[derive(Clone, Debug)]
pub struct GridLayer {
    pub origin: [f32; 2],
    /// Window-relative x, y, width, height in physical pixels.
    pub clip: [f32; 4],
    pub snapshot: TerminalSnapshot,
    /// Row/column positions are local to this snapshot.
    pub underlines: Vec<Underline>,
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub origin: [f32; 2],
    pub text: String,
    pub px: f32,
    pub color: [u8; 4],
    pub weight: u16,
    pub mono: bool,
}

impl Renderer {
    /// Draw ordered pixel-space layers. Implementation follows the API commit.
    pub fn draw_scene(&mut self, _scene: &Scene) -> Result<(), String> {
        Err("scene rendering is not implemented yet".into())
    }
}

/// Top-left row-major premultiplied RGBA8, exactly width * height * 4 bytes.
/// Implementation follows the API commit; legacy offscreen fixtures are unchanged.
pub fn offscreen_scene(_scene: &Scene, _width: u32, _height: u32) -> Result<Vec<u8>, String> {
    Err("offscreen scene rendering is not implemented yet".into())
}
