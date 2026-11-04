//! Shared window and offscreen text and image rendering.
use clanky_core::TerminalSnapshot;
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};
use wgpu::util::DeviceExt;
use winit::window::Window;

pub const CELL_WIDTH: u32 = 16;
pub const CELL_HEIGHT: u32 = 32;
const ATLAS_SIZE: u32 = 2048;
const FONT: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf");

/// Drive the native wgpu futures without an async runtime or a busy loop.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

async fn adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    let options = |fallback| wgpu::RequestAdapterOptions {
        compatible_surface: surface,
        force_fallback_adapter: fallback,
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    };
    match instance.request_adapter(&options(false)).await {
        Ok(adapter) => Ok(adapter),
        Err(first) => instance
            .request_adapter(&options(true))
            .await
            .map_err(|e| format!("No graphics adapter: {first}; fallback: {e}")),
    }
}

#[derive(Clone)]
struct Glyph {
    rect: [u32; 4],
    offset: [i32; 2],
}

struct TextPipeline {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    font_px: f32,
    cell_width: u32,
    cell_height: u32,
    atlas: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    fonts: FontSystem,
    swash: SwashCache,
    glyphs: HashMap<char, Vec<Glyph>>,
    next_x: u32,
    next_y: u32,
    shelf_height: u32,
}

impl TextPipeline {
    async fn new(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Self, String> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| e.to_string())?;
        let atlas = texture(
            &device,
            ATLAS_SIZE,
            ATLAS_SIZE,
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        // A single opaque texel supplies solid backgrounds and the cursor; glyphs
        // always use cosmic-text raster data, including U+2588.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            extent(1, 1),
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text"), layout: None,
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: 32, step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4] })],
            },
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let image_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("images"),
            source: wgpu::ShaderSource::Wgsl(include_str!("image.wgsl").into()),
        });
        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("images"), layout: None,
            vertex: wgpu::VertexState {
                module: &image_shader, entry_point: Some("vs_main"), compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: 32, step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4] })],
            },
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &image_shader, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let view = atlas.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph atlas"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        // An empty database avoids both loading and depending on host fonts.
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_data(FONT.to_vec());
        db.set_monospace_family("DejaVu Sans Mono");
        let fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        Ok(Self {
            device,
            queue,
            pipeline,
            image_pipeline,
            font_px: 16.0,
            cell_width: 10,
            cell_height: 20,
            atlas,
            bind_group,
            fonts,
            swash: SwashCache::new(),
            glyphs: HashMap::new(),
            next_x: 2,
            next_y: 0,
            shelf_height: 1,
        })
    }

    fn set_metrics(
        &mut self,
        font_px: f32,
        cell_width: u32,
        cell_height: u32,
    ) -> Result<(), String> {
        if !font_px.is_finite() || font_px <= 0.0 || cell_width == 0 || cell_height == 0 {
            return Err("Font and cell metrics must be finite and positive".into());
        }
        if (self.font_px, self.cell_width, self.cell_height) != (font_px, cell_width, cell_height) {
            self.font_px = font_px;
            self.cell_width = cell_width;
            self.cell_height = cell_height;
            self.glyphs.clear();
            self.swash = SwashCache::new();
            // Reuse atlas storage, but discard every old allocation. The solid
            // texel at (0,0) remains reserved and opaque.
            self.next_x = 2;
            self.next_y = 0;
            self.shelf_height = 1;
        }
        Ok(())
    }

    fn glyph(&mut self, ch: char) -> Result<Vec<Glyph>, String> {
        if let Some(glyphs) = self.glyphs.get(&ch) {
            return Ok(glyphs.clone());
        }
        let mut buffer = Buffer::new(
            &mut self.fonts,
            Metrics::new(self.font_px, self.cell_height as f32),
        );
        buffer.set_size(
            Some((self.cell_width as f32 * 4.0).max(64.0)),
            Some(self.cell_height as f32),
        );
        buffer.set_text(
            &ch.to_string(),
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.fonts, false);
        let mut glyphs = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                if ch == '\u{2588}' && glyph.glyph_id == 0 {
                    return Err("Bundled font has no U+2588 glyph".into());
                }
                let physical = glyph.physical((0.0, run.line_y), 1.0);
                let Some(image) = self
                    .swash
                    .get_image(&mut self.fonts, physical.cache_key)
                    .as_ref()
                else {
                    continue;
                };
                let (width, height) = (image.placement.width, image.placement.height);
                if width == 0 || height == 0 {
                    continue;
                }
                if width + 2 > ATLAS_SIZE || height + 2 > ATLAS_SIZE {
                    return Err("Glyph exceeds atlas dimensions".into());
                }
                if self.next_x + width + 1 > ATLAS_SIZE {
                    self.next_x = 0;
                    self.next_y += self.shelf_height + 1;
                    self.shelf_height = 0;
                }
                if self.next_y + height + 1 > ATLAS_SIZE {
                    return Err("Glyph atlas full".into());
                }
                let alpha: Vec<u8> = match image.content {
                    SwashContent::Mask => image.data.clone(),
                    SwashContent::Color => image.data.chunks_exact(4).map(|p| p[3]).collect(),
                    SwashContent::SubpixelMask => image
                        .data
                        .chunks_exact(4)
                        .map(|p| p[0].max(p[1]).max(p[2]))
                        .collect(),
                };
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.atlas,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: self.next_x,
                            y: self.next_y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &alpha,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width),
                        rows_per_image: Some(height),
                    },
                    extent(width, height),
                );
                glyphs.push(Glyph {
                    rect: [self.next_x, self.next_y, width, height],
                    offset: [
                        physical.x + image.placement.left,
                        physical.y - image.placement.top,
                    ],
                });
                self.next_x += width + 1;
                self.shelf_height = self.shelf_height.max(height);
            }
        }
        if ch == '\u{2588}' && glyphs.is_empty() {
            return Err("Bundled U+2588 glyph did not rasterize".into());
        }
        self.glyphs.insert(ch, glyphs.clone());
        Ok(glyphs)
    }

    fn render(
        &mut self,
        snapshot: &TerminalSnapshot,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<wgpu::CommandBuffer, String> {
        let count = snapshot
            .rows
            .checked_mul(snapshot.cols)
            .ok_or("Snapshot dimensions overflow")?;
        if count != snapshot.cells.len()
            || count > 250_000
            || snapshot.cols == 0
            || snapshot.rows == 0
        {
            return Err("Invalid terminal snapshot".into());
        }
        let mut vertices = Vec::new();
        for (index, cell) in snapshot.cells.iter().enumerate() {
            let x = (index % snapshot.cols) as f32 * self.cell_width as f32;
            let y = (index / snapshot.cols) as f32 * self.cell_height as f32;
            if x >= width as f32 || y >= height as f32 {
                continue;
            }
            quad(
                &mut vertices,
                [x, y, self.cell_width as f32, self.cell_height as f32],
                [0.5, 0.5, 0.0, 0.0],
                cell.bg,
                width,
                height,
            );
        }
        for (index, cell) in snapshot.cells.iter().enumerate() {
            let x = (index % snapshot.cols) as f32 * self.cell_width as f32;
            let y = (index / snapshot.cols) as f32 * self.cell_height as f32;
            if x >= width as f32
                || y >= height as f32
                || cell.ch.is_whitespace()
                || cell.ch.is_control()
            {
                continue;
            }
            for glyph in self.glyph(cell.ch)? {
                let [ax, ay, w, h] = glyph.rect;
                quad(
                    &mut vertices,
                    [
                        x + glyph.offset[0] as f32,
                        y + glyph.offset[1] as f32,
                        w as f32,
                        h as f32,
                    ],
                    [ax as f32, ay as f32, w as f32, h as f32],
                    cell.fg,
                    width,
                    height,
                );
            }
        }
        if let Some((row, col)) = snapshot.cursor {
            if row < snapshot.rows && col < snapshot.cols {
                quad(
                    &mut vertices,
                    [
                        col as f32 * self.cell_width as f32,
                        (row + 1) as f32 * self.cell_height as f32 - 2.0,
                        self.cell_width as f32,
                        2.0,
                    ],
                    [0.5, 0.5, 0.0, 0.0],
                    [255; 3],
                    width,
                    height,
                );
            }
        }
        let vertex_count = (vertices.len() / 32) as u32;
        // Serialize floats explicitly: no unsafe casts or extra byte-casting crate.
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terminal vertices"),
                contents: if vertices.is_empty() {
                    &[0; 32]
                } else {
                    &vertices
                },
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertex_count, 0..1);
        }
        self.render_images(snapshot, view, width, height, &mut encoder)?;
        Ok(encoder.finish())
    }

    fn render_images(
        &self,
        snapshot: &TerminalSnapshot,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), String> {
        // Share one upload across placements of the same immutable pixels.
        // Scope it to this frame so retransmitted IDs cannot reuse stale pixels.
        // Core bounds retained pixels per terminal, not across all split panes.
        let mut textures = HashMap::new();
        for image in &snapshot.images {
            let cw = self.cell_width as f32;
            let ch = self.cell_height as f32;
            let x = image.x as f32 * cw;
            let y = image.y as f32 * ch;
            let w = image.cols as f32 * cw;
            let h = image.rows as f32 * ch;
            let left = x.max(image.clip_x as f32 * cw).max(0.0);
            let top = y.max(image.clip_y as f32 * ch).max(0.0);
            let right = (x + w)
                .min(image.clip_x.saturating_add(image.clip_cols) as f32 * cw)
                .min(snapshot.cols as f32 * cw)
                .min(width as f32);
            let bottom = (y + h)
                .min(image.clip_y.saturating_add(image.clip_rows) as f32 * ch)
                .min(snapshot.rows as f32 * ch)
                .min(height as f32);
            if left >= right || top >= bottom || w <= 0.0 || h <= 0.0 {
                continue;
            }
            if image.width == 0
                || image.height == 0
                || image.width > 4096
                || image.height > 4096
                || image.width > self.device.limits().max_texture_dimension_2d
                || image.height > self.device.limits().max_texture_dimension_2d
            {
                return Err("Invalid image dimensions".into());
            }
            let size = image.width as usize * image.height as usize * 4;
            if image.rgba.len() != size {
                return Err("Invalid RGBA image length".into());
            }
            let key = (Arc::as_ptr(&image.rgba) as usize, image.width, image.height);
            let bind_group = textures.entry(key).or_insert_with(|| {
                let texture = texture(
                    &self.device,
                    image.width,
                    image.height,
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
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
                let image_view = texture.create_view(&Default::default());
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("image"),
                    layout: &self.image_pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&image_view),
                    }],
                })
            });
            let mut vertices = Vec::with_capacity(6 * 32);
            // Clip geometry and UVs together, preserving the original scale.
            quad(
                &mut vertices,
                [left, top, right - left, bottom - top],
                [
                    (left - x) / w * image.width as f32,
                    (top - y) / h * image.height as f32,
                    (right - left) / w * image.width as f32,
                    (bottom - top) / h * image.height as f32,
                ],
                [255; 3],
                width,
                height,
            );
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("image vertices"),
                    contents: &vertices,
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("image"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.image_pipeline);
            pass.set_bind_group(0, &*bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..6, 0..1);
        }
        Ok(())
    }
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
fn texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text texture"),
        size: extent(width, height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
fn quad(
    bytes: &mut Vec<u8>,
    rect: [f32; 4],
    uv: [f32; 4],
    color: [u8; 3],
    width: u32,
    height: u32,
) {
    for [s, t] in [
        [0.0, 0.0],
        [1.0, 0.0],
        [0.0, 1.0],
        [0.0, 1.0],
        [1.0, 0.0],
        [1.0, 1.0],
    ] {
        let vertex = [
            2.0 * (rect[0] + s * rect[2]) / width as f32 - 1.0,
            1.0 - 2.0 * (rect[1] + t * rect[3]) / height as f32,
            uv[0] + s * uv[2],
            uv[1] + t * uv[3],
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            1.0,
        ];
        for value in vertex {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
}

/// A surface and the same text pipeline used by the offscreen harness.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    text: TextPipeline,
    suspended: bool,
}
impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(window.clone()),
        ));
        let surface = instance.create_surface(window).map_err(|e| e.to_string())?;
        let adapter = adapter(&instance, Some(&surface)).await?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("Surface unsupported")?;
        if let Some(format) = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(|f| !f.is_srgb())
        {
            config.format = format;
        }
        let text = TextPipeline::new(&adapter, config.format).await?;
        surface.configure(&text.device, &config);
        Ok(Self {
            surface,
            config,
            text,
            suspended: size.width == 0 || size.height == 0,
        })
    }
    /// Set physical-pixel metrics after a DPI or zoom change.
    pub fn set_metrics(
        &mut self,
        font_px: f32,
        cell_width: u32,
        cell_height: u32,
    ) -> Result<(), String> {
        self.text.set_metrics(font_px, cell_width, cell_height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.suspended = width == 0 || height == 0;
        if self.suspended {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.text.device, &self.config);
    }
    pub fn draw(&mut self, snapshot: &TerminalSnapshot) -> Result<(), String> {
        if self.suspended {
            return Ok(());
        }
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
        let view = frame.texture.create_view(&Default::default());
        let commands = self
            .text
            .render(snapshot, &view, self.config.width, self.config.height)?;
        self.text.queue.submit([commands]);
        self.text.queue.present(frame);
        Ok(())
    }
}

/// Render the actual core terminal fixture and read its two declared cell centres.
pub fn offscreen_text_samples() -> Result<([u8; 4], [u8; 4]), String> {
    let snapshot = clanky_core::text_fixture();
    let [first, second] = block_on(offscreen_samples(&snapshot, [(8, 16), (24, 16)]))?;
    Ok((first, second))
}

/// Render the actual snapshot using the same image path as the window.
pub fn offscreen_image_samples(snapshot: &TerminalSnapshot) -> Result<[[u8; 4]; 4], String> {
    block_on(offscreen_samples(
        snapshot,
        [(40, 80), (56, 80), (40, 112), (56, 112)],
    ))
}

/// Render cell centres with the existing offscreen pipeline (row, column).
pub fn offscreen_cell_samples<const N: usize>(
    snapshot: &TerminalSnapshot,
    coordinates: [(usize, usize); N],
) -> Result<[[u8; 4]; N], String> {
    let mut pixels = [(0, 0); N];
    for (out, (row, col)) in pixels.iter_mut().zip(coordinates) {
        if row >= snapshot.rows || col >= snapshot.cols {
            return Err("Offscreen cell outside snapshot".into());
        }
        *out = (col * CELL_WIDTH as usize + CELL_WIDTH as usize / 2,
                row * CELL_HEIGHT as usize + CELL_HEIGHT as usize / 2);
    }
    block_on(offscreen_samples(snapshot, pixels))
}

async fn offscreen_samples<const N: usize>(
    snapshot: &TerminalSnapshot,
    coordinates: [(usize, usize); N],
) -> Result<[[u8; 4]; N], String> {
    let width = u32::try_from(snapshot.cols)
        .ok()
        .and_then(|cols| cols.checked_mul(CELL_WIDTH))
        .ok_or("Offscreen width overflow")?;
    let height = u32::try_from(snapshot.rows)
        .ok()
        .and_then(|rows| rows.checked_mul(CELL_HEIGHT))
        .ok_or("Offscreen height overflow")?;
    if width == 0
        || height == 0
        || coordinates
            .iter()
            .any(|&(x, y)| x >= width as usize || y >= height as usize)
    {
        return Err("Offscreen sample outside snapshot".into());
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = adapter(&instance, None).await?;
    let mut text = TextPipeline::new(&adapter, wgpu::TextureFormat::Rgba8Unorm).await?;
    // Both milestone fixtures retain M1's exact font and cell metrics.
    text.set_metrics(26.0, CELL_WIDTH, CELL_HEIGHT)?;
    if width > text.device.limits().max_texture_dimension_2d
        || height > text.device.limits().max_texture_dimension_2d
    {
        return Err("Offscreen target exceeds device limits".into());
    }
    let target = texture(
        &text.device,
        width,
        height,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let view = target.create_view(&Default::default());
    let draw = text.render(snapshot, &view, width, height)?;
    let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = text.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("text readback"),
        size: stride as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = text.device.create_command_encoder(&Default::default());
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
        extent(width, height),
    );
    let submission = text.queue.submit([draw, encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    text.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(20)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(1))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    // Strip row padding before applying top-left pixel coordinates.
    let pixels: Vec<u8> = mapped
        .chunks_exact(stride as usize)
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect();
    let sample = |x: usize, y: usize| -> [u8; 4] {
        let offset = (y * width as usize + x) * 4;
        pixels[offset..offset + 4].try_into().expect("RGBA pixel")
    };
    let result = coordinates.map(|(x, y)| sample(x, y));
    drop(mapped);
    readback.unmap();
    Ok(result)
}
