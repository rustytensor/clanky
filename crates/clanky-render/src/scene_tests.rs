use super::*;
use clanky_core::{Cell, ImagePlacement};
use std::sync::Arc;

fn card(rect: [f32; 4], radius: f32, fill: [u8; 4]) -> Layer {
    Layer::Shape(Shape {
        rect,
        radius,
        fill: Fill::Solid(fill),
        border: None,
        shadow: None,
    })
}
fn blank(rows: usize, cols: usize) -> TerminalSnapshot {
    TerminalSnapshot {
        rows,
        cols,
        cells: vec![
            Cell {
                ch: ' ',
                fg: [255, 255, 255],
                bg: [0, 0, 0],
                color_source: None,
            };
            rows * cols
        ],
        cursor: None,
        images: Vec::new(),
    }
}
fn image(alpha: u8) -> ImagePlacement {
    ImagePlacement {
        image_id: 1,
        placement_id: 1,
        x: 2,
        y: 2,
        cols: 2,
        rows: 2,
        width: 2,
        height: 2,
        image_key: 1,
        rgba: Arc::new(vec![
            255, 0, 0, alpha, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ]),
        clip_x: 0,
        clip_y: 0,
        clip_cols: 80,
        clip_rows: 24,
    }
}
fn fixture() -> Scene {
    let mut scene = Scene {
        clear: [15, 21, 34, 240],
        layers: vec![
            card([0., 0., 1240., 40.], 0., [11, 17, 28, 255]),
            card([0., 40., 210., 720.], 0., [11, 17, 28, 255]),
            card([970., 40., 270., 720.], 0., [11, 17, 28, 255]),
            card([220., 50., 740., 345.], 11., [19, 27, 43, 255]),
            Layer::Shape(Shape {
                rect: [220., 405., 740., 345.],
                radius: 11.,
                fill: Fill::Solid([19, 27, 43, 255]),
                border: Some((1., [143, 180, 255, 255])),
                shadow: Some(Shadow {
                    offset: [0., 0.],
                    blur: 12.,
                    spread: 0.,
                    color: [143, 180, 255, 96],
                }),
            }),
            card([228., 437., 718., 28.], 8., [255, 255, 255, 8]),
            card([904., 443., 36., 16.], 8., [95, 211, 141, 46]),
            Layer::Shape(Shape {
                rect: [100., 100., 20., 20.],
                radius: 0.,
                fill: Fill::Vertical([255, 0, 0, 255], [0, 0, 255, 255]),
                border: None,
                shadow: None,
            }),
            card([600., 42., 10., 6.], 0., [255, 0, 0, 128]),
            Layer::Text(TextRun {
                opacity: 1.0,
                origin: [20., 8.],
                text: "Clanky Studio".into(),
                px: 16.,
                color: [219, 228, 243, 255],
                weight: 600,
                mono: false,
                letter_spacing: 0.0,
            }),
        ],
    };
    let mut grid = blank(14, 71);
    grid.cells[0].ch = '█';
    grid.cells[0].fg = [255, 0, 0];
    scene.layers.push(Layer::Grid(GridLayer {
        opacity: 1.0,
        origin: [232., 441.],
        clip: [232., 441., 710., 280.],
        snapshot: grid,
        underlines: Vec::new(),
    }));
    let mut picture = blank(6, 6);
    picture.images.push(image(128));
    scene.layers.push(Layer::Grid(GridLayer {
        opacity: 1.0,
        origin: [20., 100.],
        clip: [20., 100., 30., 80.],
        snapshot: picture,
        underlines: vec![Underline {
            row: 0,
            start_col: 0,
            end_col: 2,
            color: [79, 209, 197],
        }],
    }));
    scene
}
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[(y * 1240 + x) * 4..(y * 1240 + x + 1) * 4]
        .try_into()
        .unwrap()
}
fn near(actual: [u8; 4], expected: [u8; 4], tolerance: u8, label: &str) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= tolerance),
        "{label}: expected {expected:?}, observed {actual:?}"
    );
}

/// Independent oracle: cosmic-text advances without tracking, then five gaps
/// for the six characters in Clanky. No measurement helper builds the oracle.
#[test]
fn text_metrics() {
    use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight, Wrap};
    let mut db = cosmic_text::fontdb::Database::new();
    db.load_font_data(include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf").to_vec());
    let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
    let mut run = TextRun {
        text: "Clanky".into(),
        px: 20.0,
        mono: true,
        letter_spacing: 1.5,
        ..TextRun::default()
    };
    let mut reference = Buffer::new(&mut fonts, Metrics::new(20.0, 25.0));
    reference.set_size(None, None);
    reference.set_wrap(Wrap::None);
    reference.set_text(
        &run.text,
        &Attrs::new()
            .family(Family::Name("JetBrains Mono"))
            .weight(Weight(400)),
        Shaping::Advanced,
        None,
    );
    reference.shape_until_scroll(&mut fonts, false);
    let advances: f32 = reference
        .layout_runs()
        .flat_map(|line| line.glyphs)
        .map(|g| g.w)
        .sum();
    let expected = advances + 1.5 * (run.text.chars().count() - 1) as f32;
    let layout = &reference.lines[0].layout_opt().unwrap()[0];
    let expected_vertical = (layout.max_ascent, layout.max_descent, 25.0);
    if std::env::var_os("CLANKY_TEXT_METRICS_FAULT").is_some() {
        run.letter_spacing = 0.0;
    }
    let metrics = measure_text(&run);
    assert!(
        (metrics.width - expected).abs() < 0.001,
        "width: expected {expected} (advances {advances} + 7.5), got {}",
        metrics.width
    );
    assert_eq!(
        (metrics.ascent, metrics.descent, metrics.line_height),
        expected_vertical
    );
    println!(
        "PASS: Clanky width {} = advances {} + 7.5; ascent {}, descent {}, line height {}",
        metrics.width, advances, metrics.ascent, metrics.descent, metrics.line_height
    );
}

/// On transparent clear, half opacity halves premultiplied RGB and alpha.
#[test]
fn scene_opacity() {
    let mut scene = Scene::default();
    for (index, opacity) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        let x = index as f32 * 120.;
        let mut snapshot = blank(6, 6);
        snapshot.cells[0].bg = [200, 100, 50];
        snapshot.cells[1].ch = '█';
        snapshot.cells[1].fg = [0, 255, 0];
        snapshot.cursor = Some((0, 2));
        snapshot.images.push(image(128));
        let opacity = if index == 1 && std::env::var_os("CLANKY_OPACITY_FAULT").is_some() {
            1.0
        } else {
            opacity
        };
        scene.layers.push(Layer::Grid(GridLayer {
            opacity,
            origin: [x, 0.],
            clip: [x, 0., 60., 120.],
            snapshot,
            underlines: vec![Underline {
                row: 1,
                start_col: 0,
                end_col: 1,
                color: [0, 0, 255],
            }],
        }));
        scene.layers.push(Layer::Text(TextRun {
            opacity,
            origin: [x + 80., 0.],
            text: "█".into(),
            px: 16.,
            color: [200, 100, 50, 128],
            weight: 400,
            mono: true,
            letter_spacing: 0.0,
        }));
        // The following shape must not inherit the preceding layer's fade.
        scene
            .layers
            .push(card([x + 100., 0., 10., 20.], 0., [255; 4]));
    }
    let pixels = offscreen_scene(&scene, 1240, 120).unwrap();
    for (index, factor) in [0.0_f32, 0.5, 1.0].into_iter().enumerate() {
        for (x, y, rgba, label) in [
            (5, 10, [200., 100., 50., 255.], "background fade"),
            (15, 10, [0., 255., 0., 255.], "glyph fade"),
            (25, 19, [255., 159., 90., 255.], "cursor fade"),
            (5, 38, [0., 0., 255., 255.], "underline fade"),
            (25, 50, [128., 0., 0., 128.], "image fade"),
            (
                85,
                10,
                [
                    200. * 128. / 255.,
                    100. * 128. / 255.,
                    50. * 128. / 255.,
                    128.,
                ],
                "text fade",
            ),
        ] {
            near(
                pixel(&pixels, index * 120 + x, y),
                rgba.map(|v| (v * factor).round() as u8),
                2,
                label,
            );
        }
        near(
            pixel(&pixels, index * 120 + 105, 10),
            [255; 4],
            0,
            "fade reset",
        );
    }
}

/// Expected theme samples come from design-m11-look.md, not renderer output.
/// The extra samples use straight-alpha arithmetic and literal RGBA image bytes.
#[test]
fn scene_pixels() {
    let mut scene = fixture();
    if std::env::var_os("CLANKY_SCENE_FAULT").is_some() {
        if let Layer::Shape(shape) = &mut scene.layers[3] {
            shape.radius = 0.;
        }
    }
    let pixels = offscreen_scene(&scene, 1240, 760).unwrap();
    assert_eq!(pixels.len(), 1240 * 760 * 4);
    if let Some(path) = std::env::var_os("CLANKY_SCENE_SAMPLE") {
        std::fs::write(path, &pixels).unwrap();
    }
    near(
        pixel(&pixels, 221, 51),
        [14, 20, 32, 240],
        3,
        "rounded corner",
    );
    for (point, expected, label) in [
        ((590, 44), [14, 20, 32, 240], "clear premultiplied"),
        ((226, 56), [19, 27, 43, 255], "corner interior"),
        ((950, 380), [19, 27, 43, 255], "pane padding"),
        ((100, 700), [11, 17, 28, 255], "left rail"),
        ((1100, 700), [11, 17, 28, 255], "right rail"),
        ((230, 451), [26, 34, 49, 255], "card"),
        ((936, 451), [38, 66, 66, 255], "chip"),
        (
            (300, 451),
            [26, 34, 49, 255],
            "transparent default grid background",
        ),
        ((237, 451), [255, 0, 0, 255], "bundled terminal glyph"),
        // At y=110.5 the vertical mix is 0.525: 255*(1-t), 0, 255*t.
        ((110, 110), [121, 0, 134, 255], "gradient"),
        (
            (605, 45),
            [135, 10, 16, 248],
            "translucent premultiplied layer",
        ),
        ((45, 150), [133, 8, 14, 255], "half-alpha image"),
        ((45, 170), [0, 0, 255, 255], "image origin"),
        ((55, 150), [11, 17, 28, 255], "image pane clip"),
        ((25, 118), [79, 209, 197, 255], "local underline"),
    ] {
        near(pixel(&pixels, point.0, point.1), expected, 3, label);
    }
    let focus = pixel(&pixels, 215, 577);
    assert!(
        focus[2] >= 40 && focus[2] > focus[0],
        "focus glow: {focus:?}"
    );
    assert!(
        pixels
            .chunks_exact(4)
            .all(|p| p[..3].iter().all(|c| *c <= p[3])),
        "non-premultiplied output"
    );
    if let Layer::Shape(a) = &mut scene.layers[3] {
        a.shadow = Some(Shadow {
            offset: [0., 0.],
            blur: 12.,
            spread: 0.,
            color: [167, 139, 250, 255],
        });
        a.border = Some((1., [167, 139, 250, 255]));
    }
    let agent = offscreen_scene(&scene, 1240, 760).unwrap();
    let glow = pixel(&agent, 215, 222);
    assert!(
        i16::from(glow[0]) - i16::from(glow[1]) >= 20
            && i16::from(glow[2]) - i16::from(glow[1]) >= 20,
        "agent glow: {glow:?}"
    );
    // Existing fixtures still use DejaVu at 26 px in fixed 16x32 cells.
    let (red, black) = crate::offscreen_text_samples().unwrap();
    near(red, [255, 0, 0, 255], 8, "legacy text");
    near(black, [0, 0, 0, 255], 8, "legacy neighbour");
    let mut old = blank(24, 80);
    old.images.push(image(255));
    let samples = crate::offscreen_image_samples(&old).unwrap();
    for (actual, expected) in samples.into_iter().zip([
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ]) {
        near(actual, expected, 8, "legacy image");
    }
}
