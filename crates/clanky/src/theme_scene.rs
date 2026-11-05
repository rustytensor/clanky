//! Theme-owned window backdrop. Draw content after this scene's initial layers.
use crate::themes::Theme;
use clanky_render::{Fill, Layer, Scene, Shape};

/// Cover the entire physical window, including its sidebar and exposed margins.
/// Gradient interpolation uses pixel centres and the window's full height.
pub fn window_background(theme: &Theme, window: [u32; 2], opacity: f32) -> Scene {
    let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 };
    let fade = |mut color: [u8; 4]| {
        color[3] = (color[3] as f32 * opacity).round() as u8;
        color
    };
    match &theme.window_gradient {
        None => Scene { clear: fade(theme.colors.sidebar), layers: Vec::new() },
        Some(stops) => Scene {
            // Do not blend a translucent gradient over a second coloured backdrop.
            clear: [0; 4],
            layers: vec![Layer::Shape(Shape {
                rect: [0.0, 0.0, window[0] as f32, window[1] as f32],
                radius: 0.0,
                fill: Fill::VerticalStops(stops.iter().copied().map(fade).collect()),
                border: None,
                shadow: None,
            })],
        },
    }
}
