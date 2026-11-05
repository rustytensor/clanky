//! Lyon paths and small homogeneous transforms for icons and projected panels.
//! No scene graph or 3D engine: callers supply draw order and panel transforms.
pub use lyon;
use lyon::path::{Event, Path};
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};

/// Row-major matrix acting on column vectors. Coordinates stay in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4(pub [[f64; 4]; 4]);
impl Default for Matrix4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl Matrix4 {
    pub const IDENTITY: Self = Self([
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]);
    /// Compose transformations: `a.then(b)` applies a first, then b.
    pub fn then(self, next: Self) -> Self {
        Self(std::array::from_fn(|r| {
            std::array::from_fn(|c| (0..4).map(|k| next.0[r][k] * self.0[k][c]).sum())
        }))
    }
    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        let mut m = Self::IDENTITY;
        m.0[0][3] = f64::from(x);
        m.0[1][3] = f64::from(y);
        m.0[2][3] = f64::from(z);
        m
    }
    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        let mut m = Self::IDENTITY;
        m.0[0][0] = f64::from(x);
        m.0[1][1] = f64::from(y);
        m.0[2][2] = f64::from(z);
        m
    }
    pub fn rotate_x(degrees: f32) -> Self {
        let (s, c) = f64::from(degrees).to_radians().sin_cos();
        Self([
            [1., 0., 0., 0.],
            [0., c, -s, 0.],
            [0., s, c, 0.],
            [0., 0., 0., 1.],
        ])
    }
    pub fn rotate_y(degrees: f32) -> Self {
        let (s, c) = f64::from(degrees).to_radians().sin_cos();
        Self([
            [c, 0., s, 0.],
            [0., 1., 0., 0.],
            [-s, 0., c, 0.],
            [0., 0., 0., 1.],
        ])
    }
    pub fn rotate_z(degrees: f32) -> Self {
        let (s, c) = f64::from(degrees).to_radians().sin_cos();
        Self([
            [c, -s, 0., 0.],
            [s, c, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ])
    }
    /// CSS-style perspective about a pixel-space origin: w = 1 - z/distance.
    pub fn perspective(distance: f32, origin: [f32; 2]) -> Self {
        let mut m = Self::IDENTITY;
        m.0[3][2] = -1.0 / f64::from(distance);
        Self::translate(-origin[0], -origin[1], 0.)
            .then(m)
            .then(Self::translate(origin[0], origin[1], 0.))
    }
    pub fn project(self, point: [f32; 3]) -> Option<[f32; 2]> {
        let p = [
            f64::from(point[0]),
            f64::from(point[1]),
            f64::from(point[2]),
            1.,
        ];
        let q: [f64; 4] = std::array::from_fn(|r| (0..4).map(|c| self.0[r][c] * p[c]).sum());
        if !q.iter().all(|v| v.is_finite()) || q[3] <= 0.001 {
            return None;
        }
        let out = [(q[0] / q[3]) as f32, (q[1] / q[3]) as f32];
        out.iter().all(|v| v.is_finite()).then_some(out)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PathStroke {
    pub width: f32,
    pub color: [u8; 4],
}
#[derive(Clone, Debug)]
pub struct VectorPath {
    pub path: Path,
    pub fill: Option<[u8; 4]>,
    /// Round caps and joins, matching the approved line icons and robot curves.
    pub stroke: Option<PathStroke>,
    pub transform: Matrix4,
}
#[derive(Clone, Debug)]
pub struct VectorLayer {
    pub paths: Vec<VectorPath>,
    pub clip: [f32; 4],
    pub opacity: f32,
}

pub(crate) fn vertices(
    layer: &VectorLayer,
    size: [u32; 2],
    budget: usize,
) -> Result<Vec<u8>, String> {
    if layer.paths.len() > 4096
        || !layer.clip.iter().all(|v| v.is_finite())
        || layer.clip[2] < 0.
        || layer.clip[3] < 0.
        || !(layer.clip[0] + layer.clip[2]).is_finite()
        || !(layer.clip[1] + layer.clip[3]).is_finite()
        || !layer.opacity.is_finite()
        || !(0.0..=1.0).contains(&layer.opacity)
    {
        return Err("Invalid vector layer".into());
    }
    let mut out = Vec::new();
    let mut fill = FillTessellator::new();
    let mut stroke = StrokeTessellator::new();
    for item in &layer.paths {
        if !item.transform.0.iter().flatten().all(|v| v.is_finite()) {
            return Err("Invalid vector transform".into());
        }
        // Bound input before asking lyon to allocate or flatten curves.
        let mut events = 0;
        for event in item.path.iter() {
            events += 1;
            let points = match event {
                Event::Begin { at } => vec![at],
                Event::Line { from, to }
                | Event::End {
                    last: from,
                    first: to,
                    ..
                } => vec![from, to],
                Event::Quadratic { from, ctrl, to } => vec![from, ctrl, to],
                Event::Cubic {
                    from,
                    ctrl1,
                    ctrl2,
                    to,
                } => vec![from, ctrl1, ctrl2, to],
            };
            if events > 65536
                || points.iter().any(|p| {
                    !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 65536. || p.y.abs() > 65536.
                })
            {
                return Err("Invalid or oversized vector path".into());
            }
        }
        if let Some(color) = item.fill {
            let mut mesh: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
            fill.tessellate_path(
                &item.path,
                &FillOptions::default().with_tolerance(0.1),
                &mut BuffersBuilder::new(&mut mesh, |v: FillVertex| v.position().to_array()),
            )
            .map_err(|_| "Vector fill tessellation failed")?;
            append(&mut out, &mesh, item.transform, color, layer, size, budget)?;
        }
        if let Some(style) = item.stroke {
            if !style.width.is_finite() || !(0.0..=1024.0).contains(&style.width) {
                return Err("Invalid vector stroke".into());
            }
            if style.width == 0. {
                continue;
            }
            let mut mesh: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
            let options = StrokeOptions::default()
                .with_line_width(style.width)
                .with_tolerance(0.1)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round);
            stroke
                .tessellate_path(
                    &item.path,
                    &options,
                    &mut BuffersBuilder::new(&mut mesh, |v: StrokeVertex| v.position().to_array()),
                )
                .map_err(|_| "Vector stroke tessellation failed")?;
            append(
                &mut out,
                &mesh,
                item.transform,
                style.color,
                layer,
                size,
                budget,
            )?;
        }
    }
    Ok(out)
}
fn append(
    out: &mut Vec<u8>,
    mesh: &VertexBuffers<[f32; 2], u32>,
    transform: Matrix4,
    color: [u8; 4],
    layer: &VectorLayer,
    size: [u32; 2],
    budget: usize,
) -> Result<(), String> {
    let bytes = mesh
        .indices
        .len()
        .checked_mul(40)
        .ok_or("Vector vertex budget exceeded")?;
    if out.len().saturating_add(bytes) > budget {
        return Err("Vector vertex budget exceeded".into());
    }
    let color = crate::scene_gpu::premul(color).map(|v| v * layer.opacity);
    for &index in &mesh.indices {
        let p = mesh.vertices[index as usize];
        let p = transform
            .project([p[0], p[1], 0.])
            .ok_or("Vector path crosses the perspective plane")?;
        let position = [
            2. * p[0] / size[0] as f32 - 1.,
            1. - 2. * p[1] / size[1] as f32,
        ];
        for v in position.into_iter().chain(color).chain(layer.clip) {
            out.extend_from_slice(&v.to_ne_bytes());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layer, Scene, offscreen_scene};
    use lyon::math::point;
    #[test]
    fn path_pixels() {
        // A half-plane through the pixel centre covers two of four MSAA samples.
        // Circle centre is empty; its 3px stroke fully covers the top sample.
        let mut triangle = Path::builder();
        triangle.begin(point(0., 0.));
        triangle.line_to(point(24., 0.));
        triangle.line_to(point(0., 24.));
        triangle.close();
        let mut circle = Path::builder();
        circle.add_circle(point(48., 16.), 8., lyon::path::Winding::Positive);
        let mut color = [0, 255, 0, 255];
        if std::env::var_os("CLANKY_PATH_FAULT").is_some() {
            color = [0, 0, 255, 255];
        }
        let paths = vec![
            VectorPath {
                path: triangle.build(),
                fill: Some(color),
                stroke: None,
                transform: Matrix4::IDENTITY,
            },
            VectorPath {
                path: circle.build(),
                fill: None,
                stroke: Some(PathStroke {
                    width: 3.,
                    color: [255, 0, 0, 255],
                }),
                transform: Matrix4::IDENTITY,
            },
        ];
        let scene = Scene {
            clear: [0; 4],
            layers: vec![Layer::Vector(VectorLayer {
                paths,
                clip: [0., 0., 64., 32.],
                opacity: 1.,
            })],
        };
        let pixels = offscreen_scene(&scene, 64, 32).unwrap();
        let sample = |x: usize, y: usize| -> [u8; 4] {
            pixels[(y * 64 + x) * 4..(y * 64 + x + 1) * 4]
                .try_into()
                .unwrap()
        };
        assert_eq!(sample(4, 4), [0, 255, 0, 255]);
        assert_eq!(sample(11, 12), [0, 128, 0, 128]);
        assert_eq!(sample(48, 8), [255, 0, 0, 255]);
        assert_eq!(sample(48, 16), [0; 4]);
        // x=210 about origin 200 at z=350 with d=700 doubles to x=220.
        assert_eq!(
            Matrix4::perspective(700., [200., 200.]).project([210., 200., 350.]),
            Some([220., 200.])
        );
        println!("PASS: filled triangle, round stroke, half-coverage edge and 700px perspective");
    }
}
