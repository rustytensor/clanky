use super::*;
use clanky_core::{Cell, TerminalSnapshot};

/// Expected from the prompt contract before running: both codepoints have real
/// glyphs and visible ink inside their own 8x20 cell; the next blank cell stays
/// black. Glyph zero (.notdef) and proportional spill are not valid substitutes.
#[test]
fn prompt_icons_use_installed_fallback() {
    let system_fonts = std::env::var_os("CLANKY_FONT_FALLBACK_FAULT").is_none();
    let mut fonts = scene_fonts(system_fonts);
    let mut cache = SwashCache::new();
    let chars = ['\u{e0b0}', '\u{f07c}'];
    for ch in chars {
        let buffer = shape_buffer(
            &mut fonts,
            &ch.to_string(),
            13.,
            20.,
            400,
            "Geist Mono",
            0.,
            true,
            Some(8.),
        );
        let mut glyph_count = 0;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                glyph_count += 1;
                assert_ne!(glyph.glyph_id, 0, "missing glyph U+{:04X}", ch as u32);
                assert!(
                    fonts.db().face(glyph.font_id).unwrap().monospaced,
                    "U+{:04X} did not select an installed monospace face",
                    ch as u32
                );
                let physical = glyph.physical((0., run.line_y), 1.);
                let bitmap = cache
                    .get_image(&mut fonts, physical.cache_key)
                    .as_ref()
                    .expect("fallback glyph must rasterize");
                let rect = [
                    (physical.x + bitmap.placement.left) as f32,
                    (physical.y - bitmap.placement.top) as f32,
                    bitmap.placement.width as f32,
                    bitmap.placement.height as f32,
                ];
                assert!(bitmap.placement.width > 0 && bitmap.placement.height > 0);
                let [scale, dx, dy] = fit_cell(&[rect], [8., 20.]);
                let (left, top) = (rect[0] * scale + dx, rect[1] * scale + dy);
                assert!(
                    left >= 0.
                        && top >= 0.
                        && left + rect[2] * scale <= 8.
                        && top + rect[3] * scale <= 20.,
                    "final glyph ink must fit 8x20"
                );
            }
        }
        assert!(glyph_count > 0);
    }
    let snapshot = TerminalSnapshot {
        rows: 1,
        cols: 3,
        cursor: None,
        images: vec![],
        cells: chars
            .into_iter()
            .chain([' '])
            .map(|ch| Cell {
                ch,
                fg: [255; 3],
                bg: [0; 3],
                color_source: None,
            })
            .collect(),
    };
    let scene = Scene {
        clear: [0, 0, 0, 255],
        layers: vec![Layer::Grid(GridLayer {
            origin: [0., 0.],
            clip: [0., 0., 24., 20.],
            snapshot,
            underlines: vec![],
            opacity: 1.,
        })],
    };
    // Same Painter/fontdb path as a real window, not the bundled-only fixture path.
    let pixels = crate::block_on(crate::scene::offscreen(&scene, 24, 20, system_fonts)).unwrap();
    let ink = |col: usize| {
        (0..20)
            .flat_map(|y| (col * 8..col * 8 + 8).map(move |x| (y * 24 + x) * 4))
            .filter(|&i| pixels[i..i + 3].iter().any(|&channel| channel != 0))
            .count()
    };
    assert!(ink(0) > 0, "U+E0B0 must draw visible ink");
    assert!(ink(1) > 0, "U+F07C must draw visible ink");
    assert_eq!(ink(2), 0, "fallback must not spill into the next cell");
    println!(
        "PASS: U+E0B0 and U+F07C render in 8x20 cells ({} and {} ink pixels)",
        ink(0),
        ink(1)
    );
}
