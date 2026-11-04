//! GPU implementation isolated from the legacy grid/fixture pipeline.
use crate::{Fill, GridLayer, Layer, Scene, Shape, TextMetrics, TextRun, extent, texture};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
    Weight, Wrap,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{Arc, Weak},
};
use wgpu::util::DeviceExt;

#[cfg(test)]
#[test]
fn geist_measurements() {
    // Independent HarfBuzz hb-shape on v1.9.1: Geist U+0030 advances
    // are 663/673/683 plus 10 units kerning for each of the first nine
    // zeros; Geist Mono is 600 throughout. All use 1000 units/em.
    let text = "0000000000";
    for (weight, advance) in [(400, 663.0), (500, 673.0), (600, 683.0)] {
        let ui = TextRun { text: text.into(), px: 14.0, weight, ..Default::default() };
        let mono = TextRun { px: 13.0, mono: true, ..ui.clone() };
        let expected = if std::env::var_os("CLANKY_FONT_FAULT").is_some() { 1.0 } else { (advance * 10.0 + 90.0) * 0.014 };
        assert!((measure_text(&ui).width - expected).abs() < 0.02, "Geist weight {weight}: {:?}, expected {expected}", measure_text(&ui));
        assert!((measure_text(&mono).width - 78.0).abs() < 0.02, "Geist Mono weight {weight}");
    }
    println!("PASS: Geist and Geist Mono 400/500/600 advances match v1.9.1 font tables");
}

const ATLAS: u32 = 2048;
const MAX_VERTICES: usize = 64 * 1024 * 1024;
const MAX_IMAGES: usize = 128 * 1024 * 1024;
const ATLAS_FULL: &str = "scene glyph atlas capacity exceeded";
const FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Medium.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf"),
];

pub(super) fn premul(c: [u8; 4]) -> [f32; 4] {
    let a = c[3] as f32 / 255.0;
    [
        c[0] as f32 / 255.0 * a,
        c[1] as f32 / 255.0 * a,
        c[2] as f32 / 255.0 * a,
        a,
    ]
}
fn rect_ok(r: [f32; 4]) -> bool {
    r.iter().all(|v| v.is_finite())
        && r[2] >= 0.0
        && r[3] >= 0.0
        && (r[0] + r[2]).is_finite()
        && (r[1] + r[3]).is_finite()
}
fn intersection(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    let right = (a[0] + a[2]).min(b[0] + b[2]);
    let bottom = (a[1] + a[3]).min(b[1] + b[3]);
    (right > x && bottom > y).then_some([x, y, right - x, bottom - y])
}
fn family(name: &str) -> Family<'_> {
    match name {
        "monospace" => Family::Monospace,
        "sans-serif" => Family::SansSerif,
        "serif" => Family::Serif,
        _ => Family::Name(name),
    }
}
#[derive(Clone)]
struct Bitmap {
    rect: [u32; 4],
    left: i32,
    top: i32,
}
#[derive(Clone)]
struct Placed {
    key: CacheKey,
    x: i32,
    y: i32,
}
#[derive(Hash, PartialEq, Eq)]
struct TextKey {
    text: String,
    px: u32,
    height: u32,
    letter_spacing: u32,
    weight: u16,
    mono: bool,
}
struct Image {
    source: Weak<Vec<u8>>,
    binding: wgpu::BindGroup,
    bytes: usize,
}
type ImageKey = (usize, u32, u32);
enum Op {
    Vector(Range<u32>),
    Shape(Range<u32>),
    Glyph(Range<u32>),
    Image(Range<u32>, wgpu::BindGroup),
}
struct Frame {
    opacity: f32,
    shapes: Vec<u8>,
    textures: Vec<u8>,
    vectors: Vec<u8>,
    ops: Vec<Op>,
    width: u32,
    height: u32,
}
impl Frame {
    fn viewport(&self) -> [f32; 4] {
        [0.0, 0.0, self.width as f32, self.height as f32]
    }
    fn capacity(&self, bytes: usize) -> Result<(), String> {
        if self.shapes.len() + self.textures.len() + self.vectors.len() + bytes > MAX_VERTICES {
            Err("Scene vertex budget exceeded".into())
        } else {
            Ok(())
        }
    }
    fn shape(
        &mut self,
        rect: [f32; 4],
        metrics: [f32; 4],
        top: [f32; 4],
        bottom: [f32; 4],
        border: [f32; 4],
        clip: [f32; 4],
        bounds: [f32; 4],
    ) -> Result<(), String> {
        let Some(bounds) = intersection(bounds, self.viewport()) else {
            return Ok(());
        };
        self.capacity(6 * 112)?;
        let start = (self.shapes.len() / 112) as u32;
        for [s, t] in corners() {
            let p = [bounds[0] + s * bounds[2], bounds[1] + t * bounds[3]];
            floats(
                &mut self.shapes,
                &[
                    2.0 * p[0] / self.width as f32 - 1.0,
                    1.0 - 2.0 * p[1] / self.height as f32,
                ],
            );
            floats(&mut self.shapes, &p);
            let fade = |c: [f32; 4]| c.map(|v| v * self.opacity);
            for v in [rect, metrics, fade(top), fade(bottom), fade(border), clip] {
                floats(&mut self.shapes, &v);
            }
        }
        match self.ops.last_mut() {
            Some(Op::Shape(range)) if range.end == start => range.end += 6,
            _ => self.ops.push(Op::Shape(start..start + 6)),
        }
        Ok(())
    }
    fn quad(
        &mut self,
        rect: [f32; 4],
        uv: [f32; 4],
        color: [f32; 4],
        clip: [f32; 4],
        image: Option<wgpu::BindGroup>,
    ) -> Result<(), String> {
        if intersection(rect, clip).is_none() {
            return Ok(());
        }
        self.capacity(6 * 48)?;
        let start = (self.textures.len() / 48) as u32;
        for [s, t] in corners() {
            floats(
                &mut self.textures,
                &[
                    2.0 * (rect[0] + s * rect[2]) / self.width as f32 - 1.0,
                    1.0 - 2.0 * (rect[1] + t * rect[3]) / self.height as f32,
                    uv[0] + s * uv[2],
                    uv[1] + t * uv[3],
                ],
            );
            floats(&mut self.textures, &color.map(|v| v * self.opacity));
            floats(&mut self.textures, &clip);
        }
        if let Some(binding) = image {
            self.ops.push(Op::Image(start..start + 6, binding));
        } else {
            match self.ops.last_mut() {
                Some(Op::Glyph(range)) if range.end == start => range.end += 6,
                _ => self.ops.push(Op::Glyph(start..start + 6)),
            }
        }
        Ok(())
    }
}
fn corners() -> [[f32; 2]; 6] {
    [[0., 0.], [1., 0.], [0., 1.], [0., 1.], [1., 0.], [1., 1.]]
}
fn floats(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_ne_bytes());
    }
}

fn scene_fonts(system_fonts: bool) -> FontSystem {
    let mut db = cosmic_text::fontdb::Database::new();
    for font in FONTS {
        db.load_font_data(font.to_vec());
    }
    if system_fonts {
        let bundled: HashSet<_> = db.faces().map(|face| face.id).collect();
        db.load_system_fonts();
        // Keep the shipped faces when a host also installs these families.
        let duplicates: Vec<_> = db
            .faces()
            .filter(|face| {
                !bundled.contains(&face.id)
                    && face.families.iter().any(|(name, _)| {
                        name.eq_ignore_ascii_case("Inter")
                            || name.eq_ignore_ascii_case("JetBrains Mono")
                            || name.eq_ignore_ascii_case("Geist")
                            || name.eq_ignore_ascii_case("Geist Mono")
                    })
            })
            .map(|face| face.id)
            .collect();
        for id in duplicates {
            db.remove_face(id);
        }
    } else {
        db.set_monospace_family("Geist Mono");
        db.set_sans_serif_family("Geist");
        db.set_serif_family("DejaVu Sans Mono");
    }
    FontSystem::new_with_locale_and_db("en-US".into(), db)
}

fn valid_text(text: &TextRun) -> bool {
    text.px.is_finite()
        && (1.0..=512.0).contains(&text.px)
        && (1..=1000).contains(&text.weight)
        && text.text.len() <= 16384
        && text.letter_spacing.is_finite()
        && text.letter_spacing.abs() <= 512.0
}

// Both measurement and painting use this exact cosmic-text layout. Tracking is
// converted from physical pixels to EM only at the cosmic-text boundary.
fn shape_buffer(
    fonts: &mut FontSystem,
    text: &str,
    px: f32,
    height: f32,
    weight: u16,
    name: &str,
    letter_spacing: f32,
) -> Buffer {
    let mut buffer = Buffer::new(fonts, Metrics::new(px, height));
    buffer.set_size(None, None);
    buffer.set_wrap(Wrap::None);
    let mut attrs = Attrs::new().family(family(name)).weight(Weight(weight));
    if letter_spacing != 0.0 {
        attrs = attrs.letter_spacing(letter_spacing / px);
    }
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(fonts, false);
    buffer
}

fn measure_with_fonts(fonts: &mut FontSystem, text: &TextRun, name: &str) -> TextMetrics {
    if !valid_text(text) || text.text.is_empty() {
        return TextMetrics::default();
    }
    let buffer = shape_buffer(
        fonts,
        &text.text,
        text.px,
        text.px * 1.25,
        text.weight,
        name,
        text.letter_spacing,
    );
    let mut metrics = TextMetrics {
        line_height: text.px * 1.25,
        ..TextMetrics::default()
    };
    for line in &buffer.lines {
        for layout in line.layout_opt().into_iter().flatten() {
            // Cosmic-text adds tracking to each glyph advance, including the
            // final glyph. The reported width excludes that trailing gap.
            let trailing = if layout.glyphs.is_empty() {
                0.0
            } else {
                text.letter_spacing
            };
            metrics.width = metrics.width.max((layout.w - trailing).max(0.0));
            metrics.ascent = metrics.ascent.max(layout.max_ascent);
            metrics.descent = metrics.descent.max(layout.max_descent);
        }
    }
    metrics
}

pub(super) fn measure_text(text: &TextRun) -> TextMetrics {
    thread_local! {
        // Reuse only deterministic font/shaping caches; no system font lookup,
        // GPU, window state or observable mutation enters the free function.
        static FONTS: std::cell::RefCell<FontSystem> = std::cell::RefCell::new(scene_fonts(false));
    }
    FONTS.with(|fonts| {
        measure_with_fonts(
            &mut fonts.borrow_mut(),
            text,
            if text.mono { "Geist Mono" } else { "Geist" },
        )
    })
}

pub(super) struct Painter {
    vector_pipeline: wgpu::RenderPipeline,
    samples: u32,
    format: wgpu::TextureFormat,
    system_fonts: bool,
    multisample_target: Option<(u32, u32, wgpu::Texture)>,
    shape_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    atlas: wgpu::Texture,
    atlas_binding: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    fonts: FontSystem,
    swash: SwashCache,
    glyphs: HashMap<CacheKey, Bitmap>,
    shaped: HashMap<TextKey, Vec<Placed>>,
    images: HashMap<ImageKey, Image>,
    next: [u32; 2],
    shelf: u32,
    terminal_family: String,
    ui_family: String,
    metrics: (f32, u32, u32),
}
impl Painter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, system_fonts: bool) -> Self {
        Self::with_samples(device, format, system_fonts, 1)
    }
    fn with_samples(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        system_fonts: bool,
        samples: u32,
    ) -> Self {
        let vector_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene lyon paths"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene-vector.wgsl").into()),
        });
        let vector_pipeline = pipeline(
            device,
            format,
            &vector_shader,
            "fs_main",
            40,
            &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x4,2=>Float32x4],
            samples,
        );
        let shape_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene rounded shapes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene-shape.wgsl").into()),
        });
        let texture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene textures"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene-texture.wgsl").into()),
        });
        let shape_attributes = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x4,3=>Float32x4,4=>Float32x4,5=>Float32x4,6=>Float32x4,7=>Float32x4];
        let texture_attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x4,3=>Float32x4];
        let shape_pipeline = pipeline(
            device,
            format,
            &shape_shader,
            "fs_main",
            112,
            &shape_attributes,
            samples,
        );
        let glyph_pipeline = pipeline(
            device,
            format,
            &texture_shader,
            "fs_glyph",
            48,
            &texture_attributes,
            samples,
        );
        let image_pipeline = pipeline(
            device,
            format,
            &texture_shader,
            "fs_image",
            48,
            &texture_attributes,
            samples,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene glyph interpolation"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas = texture(
            device,
            ATLAS,
            ATLAS,
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let atlas_binding = atlas_binding(device, &glyph_pipeline, &atlas, &sampler);
        Self {
            vector_pipeline,
            samples,
            format,
            system_fonts,
            multisample_target: None,
            shape_pipeline,
            glyph_pipeline,
            image_pipeline,
            atlas,
            atlas_binding,
            sampler,
            fonts: scene_fonts(system_fonts),
            swash: SwashCache::new(),
            glyphs: HashMap::new(),
            shaped: HashMap::new(),
            images: HashMap::new(),
            next: [1, 1],
            shelf: 0,
            terminal_family: "Geist Mono".into(),
            ui_family: "Geist".into(),
            metrics: (13.0, 8, 20),
        }
    }
    fn reset_atlas(&mut self, device: &wgpu::Device) {
        self.atlas = texture(
            device,
            ATLAS,
            ATLAS,
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.atlas_binding =
            atlas_binding(device, &self.glyph_pipeline, &self.atlas, &self.sampler);
        self.glyphs.clear();
        self.shaped.clear();
        self.swash = SwashCache::new();
        self.next = [1, 1];
        self.shelf = 0;
    }
    pub fn set_families(
        &mut self,
        device: &wgpu::Device,
        terminal: &str,
        ui: &str,
    ) -> Result<(), String> {
        for name in [terminal, ui] {
            if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
                return Err("Invalid font family".into());
            }
            if !matches!(name, "sans-serif" | "monospace" | "serif")
                && !self
                    .fonts
                    .db()
                    .faces()
                    .any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
            {
                return Err(format!("Font family is not installed: {name}"));
            }
        }
        if self.terminal_family != terminal || self.ui_family != ui {
            self.terminal_family = terminal.into();
            self.ui_family = ui.into();
            self.reset_atlas(device);
        }
        Ok(())
    }
    pub fn measure_text(&mut self, text: &TextRun) -> TextMetrics {
        let name = if text.mono {
            &self.terminal_family
        } else {
            &self.ui_family
        };
        measure_with_fonts(&mut self.fonts, text, name)
    }
    fn bitmap(&mut self, queue: &wgpu::Queue, key: CacheKey) -> Result<Bitmap, String> {
        if let Some(bitmap) = self.glyphs.get(&key) {
            return Ok(bitmap.clone());
        }
        let image = self
            .swash
            .get_image(&mut self.fonts, key)
            .as_ref()
            .ok_or("Glyph could not be rasterized")?;
        let (w, h) = (image.placement.width, image.placement.height);
        if w + 2 > ATLAS || h + 2 > ATLAS {
            return Err("Glyph exceeds scene atlas dimensions".into());
        }
        if self.next[0] + w + 1 > ATLAS {
            self.next[0] = 1;
            self.next[1] += self.shelf + 1;
            self.shelf = 0;
        }
        if self.next[1] + h + 1 > ATLAS {
            return Err(ATLAS_FULL.into());
        }
        let data: Vec<u8> = match image.content {
            SwashContent::Mask => image.data.clone(),
            SwashContent::Color => image.data.chunks_exact(4).map(|p| p[3]).collect(),
            SwashContent::SubpixelMask => image
                .data
                .chunks_exact(4)
                .map(|p| p[0].max(p[1]).max(p[2]))
                .collect(),
        };
        let result = Bitmap {
            rect: [self.next[0], self.next[1], w, h],
            left: image.placement.left,
            top: image.placement.top,
        };
        if w > 0 && h > 0 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: self.next[0],
                        y: self.next[1],
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w),
                    rows_per_image: Some(h),
                },
                extent(w, h),
            );
        }
        self.next[0] += w + 1;
        self.shelf = self.shelf.max(h);
        self.glyphs.insert(key, result.clone());
        Ok(result)
    }
    fn shape_text(
        &mut self,
        text: &str,
        px: f32,
        height: f32,
        weight: u16,
        mono: bool,
        letter_spacing: f32,
    ) -> Vec<Placed> {
        let key = TextKey {
            text: text.into(),
            px: px.to_bits(),
            height: height.to_bits(),
            letter_spacing: letter_spacing.to_bits(),
            weight,
            mono,
        };
        if let Some(placed) = self.shaped.get(&key) {
            return placed.clone();
        }
        let name = if mono {
            &self.terminal_family
        } else {
            &self.ui_family
        };
        let buffer = shape_buffer(
            &mut self.fonts,
            text,
            px,
            height,
            weight,
            name,
            letter_spacing,
        );
        let mut placed = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let p = glyph.physical((0.0, run.line_y), 1.0);
                placed.push(Placed {
                    key: p.cache_key,
                    x: p.x,
                    y: p.y,
                });
            }
        }
        if self.shaped.len() >= 4096 {
            self.shaped.clear();
        }
        self.shaped.insert(key, placed.clone());
        placed
    }
    fn text(
        &mut self,
        queue: &wgpu::Queue,
        frame: &mut Frame,
        text: &TextRun,
        clip: [f32; 4],
        height: f32,
    ) -> Result<(), String> {
        if !text.origin.iter().all(|v| v.is_finite()) || !valid_text(text) {
            return Err("Invalid scene text run".into());
        }
        if text.color[3] == 0 || text.text.is_empty() {
            return Ok(());
        }
        let placed = self.shape_text(
            &text.text,
            text.px,
            height,
            text.weight,
            text.mono,
            text.letter_spacing,
        );
        for p in placed {
            // Avoid rasterizing long runs outside the target. The generous
            // margin retains overhangs and combining marks near the edge.
            let x = text.origin[0] + p.x as f32;
            let y = text.origin[1] + p.y as f32;
            let margin = text.px * 4.0;
            if x + margin < clip[0]
                || x - margin > clip[0] + clip[2]
                || y + margin < clip[1]
                || y - margin > clip[1] + clip[3]
            {
                continue;
            }
            // Spaces and controls can have no raster image.
            if self.swash.get_image(&mut self.fonts, p.key).is_none() {
                continue;
            }
            let b = self.bitmap(queue, p.key)?;
            frame.quad(
                [
                    x + b.left as f32,
                    y - b.top as f32,
                    b.rect[2] as f32,
                    b.rect[3] as f32,
                ],
                b.rect.map(|v| v as f32),
                premul(text.color),
                clip,
                None,
            )?;
        }
        Ok(())
    }
    fn shape(&self, frame: &mut Frame, shape: &Shape, clip: [f32; 4]) -> Result<(), String> {
        if !rect_ok(shape.rect) || !shape.radius.is_finite() || shape.radius < 0.0 {
            return Err("Invalid scene shape".into());
        }
        if shape.rect[2] == 0.0 || shape.rect[3] == 0.0 {
            return Ok(());
        }
        let radius = shape.radius.min(shape.rect[2].min(shape.rect[3]) * 0.5);
        if let Some(shadow) = shape.shadow {
            if !shadow.offset.iter().all(|v| v.is_finite())
                || !shadow.blur.is_finite()
                || shadow.blur < 0.0
                || shadow.blur > 1024.0
                || !shadow.spread.is_finite()
            {
                return Err("Invalid scene shadow".into());
            }
            let r = [
                shape.rect[0] + shadow.offset[0] - shadow.spread,
                shape.rect[1] + shadow.offset[1] - shadow.spread,
                shape.rect[2] + 2.0 * shadow.spread,
                shape.rect[3] + 2.0 * shadow.spread,
            ];
            if !rect_ok(r) {
                return Err("Invalid shadow extent".into());
            }
            let margin = shadow.blur + 1.0;
            let bounds = [
                r[0] - margin,
                r[1] - margin,
                r[2] + margin * 2.0,
                r[3] + margin * 2.0,
            ];
            if !rect_ok(bounds) {
                return Err("Invalid shadow extent".into());
            }
            frame.shape(
                r,
                [(radius + shadow.spread).max(0.0), radius, 1.0, shadow.blur],
                premul(shadow.color),
                premul(shadow.color),
                [0.; 4],
                shape.rect,
                bounds,
            )?;
        }
        let (width, border) = shape.border.unwrap_or((0.0, [0; 4]));
        if !width.is_finite() || width < 0.0 {
            return Err("Invalid scene border".into());
        }
        let (top, bottom) = match shape.fill {
            Fill::Solid(c) => (c, c),
            Fill::Vertical(a, b) => (a, b),
        };
        let Some(bounds) = intersection(
            [
                shape.rect[0] - 1.0,
                shape.rect[1] - 1.0,
                shape.rect[2] + 2.0,
                shape.rect[3] + 2.0,
            ],
            clip,
        ) else {
            return Ok(());
        };
        frame.shape(
            shape.rect,
            [
                radius,
                width.min(shape.rect[2].min(shape.rect[3]) * 0.5),
                0.0,
                0.0,
            ],
            premul(top),
            premul(bottom),
            premul(border),
            clip,
            bounds,
        )
    }
    fn grid(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &mut Frame,
        grid: &GridLayer,
        used: &mut HashSet<ImageKey>,
    ) -> Result<(), String> {
        let s = &grid.snapshot;
        if !grid.origin.iter().all(|v| v.is_finite())
            || !rect_ok(grid.clip)
            || s.rows == 0
            || s.cols == 0
            || s.rows.checked_mul(s.cols).filter(|n| *n <= 250000) != Some(s.cells.len())
        {
            return Err("Invalid scene grid".into());
        }
        let Some(clip) = intersection(grid.clip, frame.viewport()) else {
            return Ok(());
        };
        let (px, cw, ch) = self.metrics;
        let cw = cw as f32;
        let ch = ch as f32;
        for (index, cell) in s.cells.iter().enumerate() {
            let rect = [
                grid.origin[0] + (index % s.cols) as f32 * cw,
                grid.origin[1] + (index / s.cols) as f32 * ch,
                cw,
                ch,
            ];
            let background_source=cell.color_source.map(|source| if source.inverse {source.fg}else{source.bg});
            let default_background=cell.color_source.is_some_and(|source| !source.inverse && source.bg==clanky_core::ColorSource::Default);
            // Default backgrounds expose the themed pane/card. Explicit truecolour
            // black is opaque; metadata-free legacy grids retain their old behaviour.
            let transparent=default_background || (cell.bg==[0;3] && background_source!=Some(clanky_core::ColorSource::Truecolor));
            if intersection(rect, clip).is_none() || transparent {
                continue;
            }
            self.shape(
                frame,
                &Shape {
                    rect,
                    radius: 0.0,
                    fill: Fill::Solid([cell.bg[0], cell.bg[1], cell.bg[2], 255]),
                    border: None,
                    shadow: None,
                },
                clip,
            )?;
        }
        for (index, cell) in s.cells.iter().enumerate() {
            let origin = [
                grid.origin[0] + (index % s.cols) as f32 * cw,
                grid.origin[1] + (index / s.cols) as f32 * ch,
            ];
            if cell.ch.is_whitespace()
                || cell.ch.is_control()
                || intersection([origin[0], origin[1], cw * 2.0, ch], clip).is_none()
            {
                continue;
            }
            self.text(
                queue,
                frame,
                &TextRun {
                    opacity: 1.0, // The enclosing grid's fade is applied by Frame.
                    origin,
                    text: cell.ch.to_string(),
                    px,
                    color: [cell.fg[0], cell.fg[1], cell.fg[2], 255],
                    weight: 400,
                    mono: true,
                    letter_spacing: 0.0,
                },
                clip,
                ch,
            )?;
        }
        for line in &grid.underlines {
            if line.row >= s.rows || line.start_col > line.end_col || line.end_col > s.cols {
                return Err("Invalid grid underline".into());
            }
            let rect = [
                grid.origin[0] + line.start_col as f32 * cw,
                grid.origin[1] + (line.row + 1) as f32 * ch - 2.0,
                (line.end_col - line.start_col) as f32 * cw,
                1.0,
            ];
            self.shape(
                frame,
                &Shape {
                    rect,
                    radius: 0.0,
                    fill: Fill::Solid([line.color[0], line.color[1], line.color[2], 255]),
                    border: None,
                    shadow: None,
                },
                clip,
            )?;
        }
        for image in &s.images {
            let rect = [
                grid.origin[0] + image.x as f32 * cw,
                grid.origin[1] + image.y as f32 * ch,
                image.cols as f32 * cw,
                image.rows as f32 * ch,
            ];
            let image_clip = [
                grid.origin[0] + image.clip_x as f32 * cw,
                grid.origin[1] + image.clip_y as f32 * ch,
                image.clip_cols as f32 * cw,
                image.clip_rows as f32 * ch,
            ];
            if !rect_ok(rect) || !rect_ok(image_clip) {
                return Err("Invalid image placement".into());
            }
            let Some(clip) = intersection(clip, image_clip).and_then(|c| {
                intersection(
                    c,
                    [
                        grid.origin[0],
                        grid.origin[1],
                        s.cols as f32 * cw,
                        s.rows as f32 * ch,
                    ],
                )
            }) else {
                continue;
            };
            if intersection(rect, clip).is_none() {
                continue;
            }
            if image.width == 0
                || image.height == 0
                || image.width > 4096
                || image.height > 4096
                || image.rgba.len() != image.width as usize * image.height as usize * 4
            {
                return Err("Invalid scene image".into());
            }
            let key = (Arc::as_ptr(&image.rgba) as usize, image.width, image.height);
            if used.insert(key) {
                let bytes: usize = used
                    .iter()
                    .map(|(_, w, h)| *w as usize * *h as usize * 4)
                    .sum();
                if bytes > MAX_IMAGES {
                    return Err("Scene image budget exceeds 128 MiB".into());
                }
            }
            let valid = self
                .images
                .get(&key)
                .and_then(|i| i.source.upgrade())
                .is_some_and(|old| Arc::ptr_eq(&old, &image.rgba));
            if !valid {
                let tex = texture(
                    device,
                    image.width,
                    image.height,
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    image.rgba.as_slice(),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(image.width * 4),
                        rows_per_image: Some(image.height),
                    },
                    extent(image.width, image.height),
                );
                let view = tex.create_view(&Default::default());
                let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("scene image"),
                    layout: &self.image_pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    }],
                });
                self.images.insert(
                    key,
                    Image {
                        source: Arc::downgrade(&image.rgba),
                        binding,
                        bytes: image.rgba.len(),
                    },
                );
            }
            frame.quad(
                rect,
                [0., 0., image.width as f32, image.height as f32],
                [1.; 4],
                clip,
                Some(self.images[&key].binding.clone()),
            )?;
        }
        if let Some((row, col)) = s.cursor {
            if row >= s.rows || col >= s.cols {
                return Err("Invalid grid cursor".into());
            }
            self.shape(
                frame,
                &Shape {
                    rect: [
                        grid.origin[0] + col as f32 * cw,
                        grid.origin[1] + (row + 1) as f32 * ch - 2.0,
                        cw,
                        2.0,
                    ],
                    radius: 0.0,
                    fill: Fill::Solid([255, 159, 90, 255]),
                    border: None,
                    shadow: None,
                },
                clip,
            )?;
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        width: u32,
        height: u32,
    ) -> Result<Frame, String> {
        if scene.layers.len() > 65536 {
            return Err("Too many scene layers".into());
        }
        let mut frame = Frame {
            opacity: 1.0,
            shapes: Vec::new(),
            textures: Vec::new(),
            vectors: Vec::new(),
            ops: Vec::new(),
            width,
            height,
        };
        let mut used = HashSet::new();
        let mut text_bytes = 0usize;
        for layer in &scene.layers {
            frame.opacity = match layer {
                Layer::Shape(_) | Layer::Vector(_) => 1.0,
                Layer::Grid(grid) => grid.opacity,
                Layer::Text(text) => text.opacity,
            };
            if !frame.opacity.is_finite() || !(0.0..=1.0).contains(&frame.opacity) {
                return Err("Invalid scene layer opacity: expected finite 0..=1".into());
            }
            let clip = frame.viewport();
            match layer {
                Layer::Vector(layer) => {
                    let budget = MAX_VERTICES.saturating_sub(
                        frame.shapes.len() + frame.textures.len() + frame.vectors.len(),
                    );
                    let vertices = crate::vector::vertices(layer, [width, height], budget)?;
                    let start = (frame.vectors.len() / 40) as u32;
                    frame.vectors.extend(vertices);
                    frame
                        .ops
                        .push(Op::Vector(start..(frame.vectors.len() / 40) as u32));
                }
                Layer::Shape(shape) => self.shape(&mut frame, shape, clip)?,
                Layer::Grid(grid) => self.grid(device, queue, &mut frame, grid, &mut used)?,
                Layer::Text(text) => {
                    text_bytes = text_bytes.saturating_add(text.text.len());
                    if text_bytes > 1024 * 1024 {
                        return Err("Scene text budget exceeded".into());
                    }
                    self.text(queue, &mut frame, text, clip, text.px * 1.25)?;
                }
            }
        }
        self.images.retain(|key, _| used.contains(key));
        debug_assert!(self.images.values().map(|i| i.bytes).sum::<usize>() <= MAX_IMAGES);
        Ok(frame)
    }
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        metrics: (f32, u32, u32),
        opaque: bool,
    ) -> Result<wgpu::CommandBuffer, String> {
        validate_target(device, width, height)?;
        let samples = if scene
            .layers
            .iter()
            .any(|layer| matches!(layer, Layer::Vector(_)))
        {
            4
        } else {
            1
        };
        if u64::from(width) * u64::from(height) * 4 * u64::from(samples) > 256 * 1024 * 1024 {
            return Err("Scene antialias target exceeds limits".into());
        }
        // Switch only when entering/leaving vector scenes. Existing scenes and
        // all legacy fixtures retain their original single-sample pipeline.
        if self.samples != samples {
            let mut painter = Self::with_samples(device, self.format, self.system_fonts, samples);
            painter.set_families(device, &self.terminal_family, &self.ui_family)?;
            *self = painter;
        }
        if samples > 1
            && self
                .multisample_target
                .as_ref()
                .is_none_or(|(w, h, _)| *w != width || *h != height)
        {
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scene vector antialias target"),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.multisample_target = Some((width, height, target));
        }
        if !metrics.0.is_finite()
            || !(1.0..=512.0).contains(&metrics.0)
            || metrics.1 == 0
            || metrics.2 == 0
        {
            return Err("Invalid scene metrics".into());
        }
        if self.metrics != metrics {
            self.metrics = metrics;
            self.reset_atlas(device);
        }
        let frame = match self.prepare(device, queue, scene, width, height) {
            Err(e) if e == ATLAS_FULL => {
                self.reset_atlas(device);
                self.prepare(device, queue, scene, width, height)?
            }
            other => other?,
        };
        let make = |label, data: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: if data.is_empty() { &[0; 4] } else { data },
                usage: wgpu::BufferUsages::VERTEX,
            })
        };
        let shapes = make("scene shapes", &frame.shapes);
        let textures = make("scene textured quads", &frame.textures);
        let vectors = make("scene vector triangles", &frame.vectors);
        let aa_view = self
            .multisample_target
            .as_ref()
            .map(|(_, _, texture)| texture.create_view(&Default::default()));
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut clear = scene.clear;
        if opaque {
            clear[3] = 255;
        }
        let c = premul(clear);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: aa_view.as_ref().unwrap_or(view),
                    depth_slice: None,
                    resolve_target: aa_view.as_ref().map(|_| view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: c[0] as f64,
                            g: c[1] as f64,
                            b: c[2] as f64,
                            a: c[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for op in &frame.ops {
                match op {
                    Op::Vector(range) => {
                        pass.set_pipeline(&self.vector_pipeline);
                        pass.set_vertex_buffer(0, vectors.slice(..));
                        pass.draw(range.clone(), 0..1);
                    }
                    Op::Shape(range) => {
                        pass.set_pipeline(&self.shape_pipeline);
                        pass.set_vertex_buffer(0, shapes.slice(..));
                        pass.draw(range.clone(), 0..1);
                    }
                    Op::Glyph(range) => {
                        pass.set_pipeline(&self.glyph_pipeline);
                        pass.set_bind_group(0, &self.atlas_binding, &[]);
                        pass.set_vertex_buffer(0, textures.slice(..));
                        pass.draw(range.clone(), 0..1);
                    }
                    Op::Image(range, binding) => {
                        pass.set_pipeline(&self.image_pipeline);
                        pass.set_bind_group(0, binding, &[]);
                        pass.set_vertex_buffer(0, textures.slice(..));
                        pass.draw(range.clone(), 0..1);
                    }
                }
            }
        }
        Ok(encoder.finish())
    }
}
fn atlas_binding(
    device: &wgpu::Device,
    pipeline: &wgpu::RenderPipeline,
    atlas: &wgpu::Texture,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let view = atlas.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene glyph atlas"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
fn pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    fragment: &str,
    stride: u64,
    attributes: &[wgpu::VertexAttribute],
    samples: u32,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: None,
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes,
            })],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
pub(super) fn validate_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > device.limits().max_texture_dimension_2d
        || height > device.limits().max_texture_dimension_2d
        || u64::from(width) * u64::from(height) * 4 > 256 * 1024 * 1024
    {
        Err("Scene target dimensions exceed limits".into())
    } else {
        Ok(())
    }
}
