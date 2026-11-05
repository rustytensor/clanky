//! Pixel-space scene contract. Colours supplied by callers are straight RGBA;
//! rendered targets and readback pixels use premultiplied RGBA.
use crate::{Renderer, Underline};
use clanky_core::TerminalSnapshot;

#[cfg(test)]
#[test]
fn flat_scene_clip(){
    let fill=|rect,color|Layer::Shape(Shape{rect,radius:0.0,fill:Fill::Solid(color),border:None,shadow:None});
    let clip=if std::env::var_os("CLANKY_CLIP_FAULT").is_some(){None}else{Some([8.0,8.0,8.0,8.0])};
    let scene=Scene{clear:[255;4],layers:vec![Layer::Clip(clip),fill([0.0,0.0,32.0,32.0],[255,0,0,255]),Layer::Clip(None),fill([0.0,0.0,4.0,4.0],[0,0,255,255])]};
    let pixels=offscreen_scene(&scene,32,32).unwrap();
    for(x,y,color,label)in[(9,9,[255,0,0,255],"inside clip"),(7,9,[255;4],"outside clip"),(16,9,[255;4],"right clip edge"),(1,1,[0,0,255,255],"clip reset")]{let i=(y*32+x)*4;assert_eq!(&pixels[i..i+4],&color,"{label}");}
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub clear: [u8; 4],
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug)]
pub enum Layer {
    /// Clip subsequent layers to x/y/width/height. None restores the viewport.
    /// Flat and non-nesting; each scene starts with the full viewport.
    Clip(Option<[f32; 4]>),
    Shape(Shape),
    Grid(GridLayer),
    Text(TextRun),
    /// Ordered lyon fills/strokes, with per-path homogeneous transforms and 4x MSAA.
    Vector(crate::VectorLayer),
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

#[derive(Clone, Debug)]
pub enum Fill {
    Solid([u8; 4]),
    /// Top and bottom straight RGBA colours.
    Vertical([u8; 4], [u8; 4]),
    /// Two to eight straight RGBA stops, evenly spaced over the full rectangle.
    VerticalStops(Vec<[u8; 4]>),
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
    /// Fade each primitive (including images), finite 0..=1; 1 preserves its colour.
    pub opacity: f32,
    /// Provenance-marked default backgrounds are transparent in scenes; explicit
    /// truecolour backgrounds stay opaque, including black. RGB-only legacy grids
    /// still treat black as transparent. Legacy draw() does not change.
    pub origin: [f32; 2],
    /// Window-relative x, y, width, height in physical pixels.
    pub clip: [f32; 4],
    pub snapshot: TerminalSnapshot,
    /// Row/column positions are local to this snapshot.
    pub underlines: Vec<Underline>,
}

#[derive(Clone, Debug)]
pub struct TextRun {
    /// Fade the run, finite 0..=1; multiplied with the colour's alpha.
    pub opacity: f32,
    pub origin: [f32; 2],
    pub text: String,
    pub px: f32,
    pub color: [u8; 4],
    pub weight: u16,
    pub mono: bool,
    /// Additional tracking in physical pixels; zero preserves normal shaping.
    pub letter_spacing: f32,
}

impl Default for TextRun {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            origin: [0.0; 2],
            text: String::new(),
            px: 14.0,
            color: [255; 4],
            weight: 400,
            mono: false,
            letter_spacing: 0.0,
        }
    }
}

/// Advance width of the widest line and maximum font ascent/descent, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    /// Baseline spacing, not the total height of a multiline run.
    pub line_height: f32,
}

/// Deterministic, GPU-free measurement with the bundled scene fonts.
/// Invalid size, weight or spacing returns zero metrics, as does empty text.
/// For configured window fonts, use Renderer::measure_text instead.
pub fn measure_text(text: &TextRun) -> TextMetrics {
    crate::scene_gpu::measure_text(text)
}

impl Renderer {
    /// Measure using this window's configured font families, without drawing.
    pub fn measure_text(&mut self, text: &TextRun) -> TextMetrics {
        let painter = self.scene.get_or_insert_with(|| {
            crate::scene_gpu::Painter::new(&self.text.device, self.config.format, true)
        });
        painter.measure_text(text)
    }

    /// Whether the compositor accepts premultiplied window transparency.
    /// When false, draw_scene ignores clear alpha; the app can report this once.
    pub fn supports_transparency(&self) -> bool {
        self.config.alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied
    }

    /// Configure installed or bundled scene families. Legacy fixture fonts stay fixed.
    pub fn set_font_families(&mut self, terminal: &str, ui: &str) -> Result<(), String> {
        let painter = self.scene.get_or_insert_with(|| {
            crate::scene_gpu::Painter::new(&self.text.device, self.config.format, true)
        });
        painter.set_families(&self.text.device, terminal, ui)
    }

    /// Draw ordered pixel-space layers with straight input/premultiplied output.
    pub fn draw_scene(&mut self, scene: &Scene) -> Result<(), String> {
        if self.suspended {
            return Ok(());
        }
        let started = std::time::Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.text.device, &self.config);
                return Ok(());
            }
            other => return Err(format!("Surface acquisition failed: {other:?}")),
        };
        let acquired = std::time::Instant::now();
        let view = frame.texture.create_view(&Default::default());
        let opaque = !self.supports_transparency();
        let painter = self.scene.get_or_insert_with(|| {
            crate::scene_gpu::Painter::new(&self.text.device, self.config.format, true)
        });
        let commands = painter.render(
            &self.text.device,
            &self.text.queue,
            scene,
            &view,
            self.config.width,
            self.config.height,
            (
                self.text.font_px,
                self.text.cell_width,
                self.text.cell_height,
            ),
            opaque,
        )?;
        let encoded = std::time::Instant::now();
        self.text.queue.submit([commands]);
        let submitted = std::time::Instant::now();
        self.text.queue.present(frame);
        let micros = |a: std::time::Instant, b: std::time::Instant| (b - a).as_micros() as u64;
        self.frame_timing = [
            micros(started, acquired),
            micros(acquired, encoded),
            micros(encoded, submitted),
            micros(submitted, std::time::Instant::now()),
        ];
        Ok(())
    }
}

/// Top-left row-major premultiplied RGBA8, exactly width * height * 4 bytes.
/// Uses bundled Geist Mono at 13 px and 8x20 cells. Legacy fixtures are unchanged.
pub fn offscreen_scene(scene: &Scene, width: u32, height: u32) -> Result<Vec<u8>, String> {
    crate::block_on(offscreen(scene, width, height))
}

async fn offscreen(scene: &Scene, width: u32, height: u32) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) * 4 > 256 * 1024 * 1024 {
        return Err("Invalid offscreen scene dimensions".into());
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = crate::adapter(&instance, None).await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|e| e.to_string())?;
    crate::scene_gpu::validate_target(&device, width, height)?;
    let mut painter =
        crate::scene_gpu::Painter::new(&device, wgpu::TextureFormat::Rgba8Unorm, false);
    let target = crate::texture(
        &device,
        width,
        height,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let view = target.create_view(&Default::default());
    let draw = painter.render(
        &device,
        &queue,
        scene,
        &view,
        width,
        height,
        (13.0, 8, 20),
        false,
    )?;
    let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        crate::extent(width, height),
    );
    let submission = queue.submit([draw, encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(20)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(1))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let pixels = mapped
        .chunks_exact(stride as usize)
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect();
    drop(mapped);
    readback.unmap();
    Ok(pixels)
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
