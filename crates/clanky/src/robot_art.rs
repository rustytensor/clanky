//! Robot6 geometry, in its 400x400 reference coordinate system.
use super::{Frame, OUT, Pose, progress};
use crate::themes::{Appearance, Color, Theme};
use clanky_render::vector::lyon::{
    math::{Box2D, point},
    path::{Path, Winding, builder::BorderRadii},
};
use clanky_render::{Layer, Matrix4 as M, PathStroke, TextRun, VectorLayer, VectorPath};

fn mix(a: Color, b: Color, t: f32) -> Color {
    std::array::from_fn(|i| (a[i] as f32 * (1. - t) + b[i] as f32 * t).round() as u8)
}
fn alpha(mut c: Color, a: f32) -> Color {
    c[3] = (c[3] as f32 * a.clamp(0., 1.)).round() as u8;
    c
}
fn bright(mut c: Color, b: f32) -> Color {
    for v in &mut c[..3] {
        *v = (*v as f32 * b).round().clamp(0., 255.) as u8;
    }
    c
}
#[derive(Clone, Copy)]
struct Paint {
    metal: Color,
    metal2: Color,
    edge: Color,
    line: Color,
    tread: Color,
    tread2: Color,
    eye: Color,
    lens: Color,
    ink: Color,
    paint: Color,
    paintline: Color,
    scuff: Color,
    shadow: Color,
}
impl Paint {
    fn new(t: &Theme) -> Self {
        let c = &t.colors;
        let light = t.appearance == Appearance::Light;
        Self {
            metal: mix(c.muted, c.muted_foreground, if light { 0.12 } else { 0.16 }),
            metal2: if light {
                mix(c.muted, c.muted_foreground, 0.24)
            } else {
                c.accent
            },
            edge: c.muted_foreground,
            line: if light {
                c.ring
            } else {
                mix(c.muted, c.muted_foreground, 0.42)
            },
            tread: if light {
                mix(c.foreground, c.muted_foreground, 0.3)
            } else {
                c.card
            },
            tread2: if light {
                mix(c.foreground, c.muted_foreground, 0.46)
            } else {
                c.accent
            },
            eye: c.agent,
            lens: if light { c.foreground } else { c.sidebar },
            ink: c.foreground,
            paint: c.robot_paint,
            paintline: mix(c.robot_paint, c.muted_foreground, 0.6),
            scuff: mix(c.robot_paint, c.muted_foreground, 0.4),
            shadow: alpha(c.foreground, if light { 0.18 } else { 0. }),
        }
    }
}
#[derive(Default)]
struct Art {
    paths: Vec<VectorPath>,
    m: M,
}
impl Art {
    fn scope(&mut self, m: M, f: impl FnOnce(&mut Self)) {
        let old = self.m;
        self.m = m.then(old);
        f(self);
        self.m = old;
    }
    fn add(&mut self, path: Path, fill: Option<Color>, stroke: Option<(f32, Color)>) {
        self.paths.push(VectorPath {
            path,
            fill,
            stroke: stroke.map(|(width, color)| PathStroke { width, color }),
            transform: self.m,
        });
    }
    fn rect(&mut self, r: [f32; 4], radius: f32, fill: Color, stroke: Option<(f32, Color)>) {
        let mut b = Path::builder();
        b.add_rounded_rectangle(
            &Box2D::new(point(r[0], r[1]), point(r[0] + r[2], r[1] + r[3])),
            &BorderRadii::new(radius),
            Winding::Positive,
        );
        self.add(b.build(), Some(fill), stroke);
    }
    fn circle(&mut self, p: [f32; 2], r: f32, fill: Option<Color>, stroke: Option<(f32, Color)>) {
        let mut b = Path::builder();
        b.add_circle(point(p[0], p[1]), r, Winding::Positive);
        self.add(b.build(), fill, stroke);
    }
    fn ellipse(&mut self, center: [f32; 2], radii: [f32; 2], fill: Color) {
        let mut builder = Path::builder();
        builder.add_ellipse(
            point(center[0], center[1]),
            clanky_render::vector::lyon::math::vector(radii[0], radii[1]),
            clanky_render::vector::lyon::math::Angle::radians(0.),
            Winding::Positive,
        );
        self.add(builder.build(), Some(fill), None);
    }
    fn line(&mut self, points: &[[f32; 2]], width: f32, color: Color) {
        let mut b = Path::builder();
        b.begin(point(points[0][0], points[0][1]));
        for p in &points[1..] {
            b.line_to(point(p[0], p[1]));
        }
        b.end(false);
        self.add(b.build(), None, Some((width, color)));
    }
}
fn around(p: [f32; 2], m: M) -> M {
    M::translate(-p[0], -p[1], 0.)
        .then(m)
        .then(M::translate(p[0], p[1], 0.))
}
fn arm(a: &mut Art, p: Paint, right: bool, pose: &Pose) {
    let shoulder = if right { [256., 190.] } else { [144., 190.] };
    let angle = if right { -30. + pose.wave } else { 30. };
    a.scope(
        M::rotate_z(angle).then(M::translate(shoulder[0], shoulder[1], 0.)),
        |a| {
            a.rect([-8., 0., 16., 42.], 8., p.metal, Some((2.2, p.edge)));
            a.line(
                &[
                    [if right { 3. } else { -3. }, 10.],
                    [if right { 3. } else { -3. }, 32.],
                ],
                1.5,
                p.line,
            );
            a.scope(
                M::rotate_z(if right { 42. + pose.elbow } else { -42. })
                    .then(M::translate(0., 40., 0.)),
                |a| {
                    a.circle([0., 0.], 8., Some(p.metal2), Some((2., p.edge)));
                    a.circle([0., 0.], 2.5, Some(p.edge), None);
                    a.rect([-6.5, 4., 13., 32.], 6., p.metal, Some((2.2, p.edge)));
                    a.scope(M::translate(0., 36., 0.), |a| {
                        a.rect([-10., -2., 20., 10.], 3., p.metal2, Some((2., p.edge)));
                        a.scope(
                            around(
                                [-7., 7.],
                                M::rotate_z(14. + if right { pose.claw } else { 0. }),
                            ),
                            |a| a.rect([-11., 6., 7., 14.], 3.5, p.metal2, Some((2., p.edge))),
                        );
                        a.scope(around([7., 7.], M::rotate_z(-14.)), |a| {
                            a.rect([4., 6., 7., 14.], 3.5, p.metal2, Some((2., p.edge)))
                        });
                    });
                },
            );
        },
    );
}
fn base_side(p: Paint, pose: &Pose) -> Art {
    let mut a = Art::default();
    a.rect(
        [1.25, 1.25, 189.5, 55.5],
        27.75,
        p.tread,
        Some((2.5, p.edge)),
    );
    // Belt segments travel round the same rounded tread perimeter as the wheels.
    for i in 0..28 {
        let q = (i as f32 * 12. + pose.wheel.abs() * 0.2) % 336.;
        let (x, y) = if q < 134. {
            (29. + q, 8.)
        } else if q < 168. {
            (176., 8. + (q - 134.))
        } else if q < 302. {
            (163. - (q - 168.), 50.)
        } else {
            (8., 42. - (q - 302.))
        };
        a.line(
            &[
                [x, y],
                [
                    if q < 134. || (168. ..302.).contains(&q) {
                        x + 5.
                    } else {
                        x
                    },
                    if q < 134. || (168. ..302.).contains(&q) {
                        y
                    } else {
                        y + 5.
                    },
                ],
            ],
            1.5,
            p.line,
        );
    }
    for (x, y, r, hub) in [
        (30., 29., 17., 4.),
        (96., 33., 13., 3.),
        (162., 29., 17., 4.),
    ] {
        a.circle([x, y], r, Some(p.metal2), Some((2., p.edge)));
        a.scope(around([x, y], M::rotate_z(pose.wheel)), |a| {
            a.line(&[[x, y - r + 2.], [x, y + r - 2.]], 2., p.line);
            a.line(&[[x - r + 2., y], [x + r - 2., y]], 2., p.line);
        });
        a.circle([x, y], hub, Some(p.edge), None);
    }
    a
}
fn base_front(p: Paint) -> Art {
    let mut a = Art::default();
    for x in [2., 84.] {
        a.rect([x, 1.25, 34., 55.5], 11., p.tread, Some((2.5, p.edge)));
        for y in [12., 20., 28., 36., 44.] {
            a.line(&[[x + 6., y], [x + 28., y]], 2., p.line);
        }
    }
    a.rect([34., 16., 52., 28.], 7., p.metal2, Some((2., p.edge)));
    a.circle([60., 30.], 5., Some(p.eye), None);
    for x in [44., 76.] {
        a.circle([x, 30.], 2., Some(p.edge), None);
    }
    a
}
fn base_top(p: Paint) -> Art {
    let mut a = Art::default();
    for y in [1., 83.] {
        a.rect([1., y, 190., 36.], 12., p.tread2, Some((2., p.edge)));
        for i in 0..11 {
            let x = 14. + i as f32 * 16.;
            a.line(&[[x, y + 9.], [x, y + 27.]], 2., p.line);
        }
    }
    a.rect([40., 37., 112., 46.], 6., p.metal2, Some((2., p.edge)));
    a
}
fn torso_front(p: Paint, pose: &Pose) -> Art {
    let mut a = Art {
        m: M::translate(-148., -166., 0.),
        ..Art::default()
    };
    arm(&mut a, p, false, pose);
    a.rect(
        [149.25, 167.25, 101.5, 93.5],
        12.,
        p.metal,
        Some((2.5, p.edge)),
    );
    a.rect([156., 176., 88., 62.], 8., p.paint, Some((1.8, p.edge)));
    a.line(&[[156., 207.], [244., 207.]], 1.2, p.paintline);
    for points in [
        [[162., 232.], [169., 230.]],
        [[231., 181.], [237., 184.]],
        [[218., 234.], [227., 233.]],
    ] {
        a.line(&points, 1.2, p.scuff);
    }
    a.rect([162., 182., 20., 38.], 3., p.lens, Some((1.5, p.edge)));
    for i in 0..5 {
        let y = 213. - i as f32 * 7.;
        a.rect([165., y, 14., 5.], 1.5, p.line, None);
        a.rect([165., y, 14., 5.], 1.5, alpha(p.eye, pose.bars[i]), None);
    }
    a.rect([196., 182., 40., 22.], 4., p.lens, Some((1.5, p.edge)));
    a.line(
        &[
            [201., 196.],
            [206., 191.],
            [211., 196.],
            [216., 188.],
            [221., 192.],
            [226., 190.],
            [230., 193.],
        ],
        1.8,
        alpha(bright(p.eye, pose.flash), pose.light),
    );
    for x in [164., 190., 216.] {
        a.line(&[[x, 248.], [x + 18., 248.]], 3., p.line);
    }
    for x in [154., 246.] {
        for y in [172., 256.] {
            a.circle([x, y], 2., Some(p.edge), None);
        }
    }
    for x in [146., 254.] {
        a.circle([x, 190.], 9., Some(p.metal2), Some((2., p.edge)));
        a.circle([x, 190.], 3., Some(p.edge), None);
    }
    arm(&mut a, p, true, pose);
    a
}
fn torso_side(p: Paint) -> Art {
    let mut a = Art::default();
    a.rect([1.25, 1.25, 61.5, 93.5], 12., p.metal, Some((2.5, p.edge)));
    a.rect([40., 10., 18., 62.], 5., p.paint, Some((1.5, p.edge)));
    a.line(&[[10., 82.], [50., 82.]], 3., p.line);
    for y in [8., 88.] {
        a.circle([8., y], 2., Some(p.edge), None);
    }
    a.scope(M::translate(30., 24., 0.), |a| {
        a.circle([0., 0.], 9., Some(p.metal2), Some((2., p.edge)));
        a.circle([0., 0.], 3., Some(p.edge), None);
        a.rect([-8., 4., 16., 40.], 8., p.metal, Some((2.2, p.edge)));
        a.scope(M::rotate_z(-35.).then(M::translate(0., 44., 0.)), |a| {
            a.circle([0., 0.], 7., Some(p.metal2), Some((2., p.edge)));
            a.rect([-6.5, 3., 13., 30.], 6., p.metal, Some((2.2, p.edge)));
            a.rect([-9., 31., 18., 9.], 3., p.metal2, Some((2., p.edge)));
            a.scope(around([2., 40.], M::rotate_z(-18.)), |a| {
                a.rect([-1., 38., 7., 13.], 3.5, p.metal2, Some((2., p.edge)))
            });
        });
    });
    a
}
fn head_front(p: Paint, pose: &Pose) -> Art {
    let mut a = Art {
        m: M::translate(-146., -80., 0.),
        ..Art::default()
    };
    a.line(&[[200., 80.], [200., 66.]], 2.5, p.edge);
    a.circle([200., 62.], 4., Some(p.line), None);
    a.circle(
        [200., 62.],
        4.,
        Some(alpha(bright(p.eye, pose.flash), pose.light)),
        None,
    );
    a.circle(
        [200., 62.],
        4. * pose.ping_size,
        None,
        Some((1.5, alpha(p.eye, pose.ping))),
    );
    a.rect([150., 120., 100., 14.], 6., p.metal2, Some((2., p.edge)));
    for (x, cx, angle) in [(147.25, 171., pose.frown), (205.25, 229., -pose.frown)] {
        a.rect([x, 81.25, 47.5, 41.5], 14., p.metal, Some((2.5, p.edge)));
        a.circle([cx, 102.], 15., Some(p.lens), Some((2., p.edge)));
        a.circle([cx, 102.], 7., Some(p.line), None);
        // The reference's .lit animation overrides the SVG opacity attribute.
        a.circle(
            [cx, 102.],
            15.,
            Some(alpha(bright(p.eye, pose.flash), pose.light)),
            None,
        );
        a.circle(
            [cx, 102.],
            7. * pose.iris,
            Some(alpha(bright(p.eye, pose.flash), pose.light)),
            None,
        );
        a.circle([cx - 4., 98.], 2.5, Some(alpha(p.ink, 0.8)), None);
        a.scope(
            around(
                [cx, 83.],
                M::rotate_z(angle).then(M::scale(1., pose.lids, 1.)),
            ),
            |a| a.rect([cx - 19., 83., 38., 19.], 8., p.metal2, Some((1.5, p.edge))),
        );
    }
    a
}
fn head_side(p: Paint) -> Art {
    let mut a = Art::default();
    a.line(&[[25., 0.], [25., -14.]], 2.5, p.edge);
    a.circle([25., -18.], 4., Some(p.line), None);
    a.rect([4., 40., 40., 14.], 6., p.metal2, Some((2., p.edge)));
    a.rect([1.25, 1.25, 44., 41.5], 13., p.metal, Some((2.5, p.edge)));
    a.rect([38., 7., 11., 30.], 5., p.metal2, Some((2., p.edge)));
    a.rect([45., 11., 5., 22.], 2.5, p.lens, Some((1.5, p.edge)));
    for y in [14., 22., 30.] {
        a.line(&[[8., y], [30., y]], 2., p.line);
    }
    a
}
fn blank(w: f32, h: f32, p: Paint) -> Art {
    let mut a = Art::default();
    a.rect([1., 1., w - 2., h - 2.], 10., p.metal2, Some((2., p.edge)));
    a
}

/// A box is six independently projected flat paths. Visible faces are sorted
/// back-to-front; there is no depth buffer or hidden 3D engine.
fn face_light(time: f32, yaw: f32, start: f32, duration: f32, front_shade: f32) -> [f32; 2] {
    if yaw == -90. {
        return [0.45, 1.];
    }
    [
        1. - 0.55 * progress(time, start, duration, [0.42, 0., 1., 1.]),
        1. - front_shade * (1. - progress(time, start, duration, OUT)),
    ]
}
fn box_paths(
    out: &mut Art,
    w: f32,
    h: f32,
    d: f32,
    model: M,
    side: Art,
    front: Art,
    top: Art,
    p: Paint,
    light: [f32; 2],
    yaw: f32,
) -> M {
    let side_m = M::translate(-w * 0.5, -h * 0.5, d * 0.5);
    let front_m = M::translate(-d * 0.5, -h * 0.5, 0.)
        .then(M::rotate_y(90.))
        .then(M::translate(w * 0.5, 0., 0.));
    let top_m = M::translate(-w * 0.5, -d * 0.5, 0.)
        .then(M::rotate_x(90.))
        .then(M::translate(0., -h * 0.5, 0.));
    let mut faces = vec![
        (side, side_m, w, h, light[0]),
        (front, front_m, d, h, light[1]),
        (top, top_m, w, d, 1.),
        (
            blank(w, h, p),
            M::translate(-w * 0.5, -h * 0.5, 0.)
                .then(M::rotate_y(180.))
                .then(M::translate(0., 0., -d * 0.5)),
            w,
            h,
            0.65,
        ),
        (
            blank(d, h, p),
            M::translate(-d * 0.5, -h * 0.5, 0.)
                .then(M::rotate_y(-90.))
                .then(M::translate(-w * 0.5, 0., 0.)),
            d,
            h,
            0.65,
        ),
        (
            blank(w, d, p),
            M::translate(-w * 0.5, -d * 0.5, 0.)
                .then(M::rotate_x(-90.))
                .then(M::translate(0., h * 0.5, 0.)),
            w,
            d,
            0.6,
        ),
    ];
    // Outside a turn the reference is deliberately flat, not a box with a
    // visible underside caused by the off-centre perspective origin.
    if yaw == 0. {
        faces = vec![faces.remove(0)];
    } else if yaw == -90. {
        faces = vec![faces.remove(1)];
    }
    faces.sort_by(|a, b| {
        let z = |m: M, w: f32, h: f32| {
            let m = m.then(model);
            m.0[2][0] * w as f64 * 0.5 + m.0[2][1] * h as f64 * 0.5 + m.0[2][3]
        };
        z(a.1, a.2, a.3).total_cmp(&z(b.1, b.2, b.3))
    });
    for (face, m, w, h, light) in faces {
        let m = m.then(model);
        let Some(a) = m.project([0., 0., 0.]) else {
            continue;
        };
        let Some(b) = m.project([w, 0., 0.]) else {
            continue;
        };
        let Some(c) = m.project([0., h, 0.]) else {
            continue;
        };
        if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) <= 0.01 {
            continue;
        }
        for mut path in face.paths {
            path.transform = path.transform.then(m);
            path.fill = path.fill.map(|c| bright(c, light));
            if let Some(stroke) = &mut path.stroke {
                stroke.color = bright(stroke.color, light);
            }
            out.paths.push(path);
        }
    }
    front_m.then(model)
}

pub(super) fn draw(
    theme: &Theme,
    rect: [f32; 4],
    pose: &Pose,
    measure: &mut dyn FnMut(&TextRun) -> clanky_render::TextMetrics,
) -> Frame {
    let p = Paint::new(theme);
    let scale = (rect[2] / 400.).min(rect[3] / 400.).max(0.);
    let screen = M::scale(scale, scale, 1.).then(M::translate(
        rect[0] + (rect[2] - 400. * scale) * 0.5,
        rect[1] + (rect[3] - 400. * scale) * 0.5,
        0.,
    ));
    let root = M::translate(pose.roll, 0., 0.)
        .then(around([200., 260.], M::scale(pose.camera, pose.camera, 1.)))
        .then(M::perspective(700., [200., 200.]))
        .then(screen);
    let body = around([200., 268.], M::rotate_z(pose.brake)).then(M::translate(0., pose.bob, 0.));
    let mut a = Art::default();
    // Soft bloom and floor shadow are layered translucent ellipses, all theme-derived.
    for i in (1..=16).rev() {
        let r = i as f32 * 6.;
        a.scope(screen, |a| {
            a.circle(
                [200., 190.],
                r,
                Some(alpha(p.eye, pose.bloom * 0.014)),
                None,
            )
        });
    }
    let (shadow_x, shadow_y) = if pose.base_yaw == -90. {
        (0.66, 1.25)
    } else {
        (
            super::keys(
                pose.time,
                5450.,
                800.,
                super::IN_OUT,
                &[(0., 1.), (0.5, 0.86), (1., 0.66)],
            ),
            super::keys(
                pose.time,
                5450.,
                800.,
                super::IN_OUT,
                &[(0., 1.), (0.5, 1.6), (1., 1.25)],
            ),
        )
    };
    for i in (0..8).rev() {
        let blur = i as f32 * 0.3;
        let shadow = if theme.appearance == Appearance::Dark {
            alpha(bright(theme.colors.sidebar, 0.), 0.083)
        } else {
            alpha(p.shadow, 0.125)
        };
        a.scope(root, |a| {
            a.ellipse(
                [200., 334.],
                [120. * shadow_x + blur, 8. * shadow_y + blur],
                shadow,
            )
        });
    }
    for (x, y, r, start, duration) in [
        (300., 318., 7., 1550., 900.),
        (314., 312., 5., 1600., 850.),
        (288., 322., 4.5, 1650., 800.),
    ] {
        let q = progress(pose.time, start, duration, OUT);
        let opacity = if q < 0.25 {
            q * 3.6
        } else {
            0.9 * (1. - q) / 0.75
        };
        a.scope(root, |a| {
            a.circle(
                [x + 26. * q, y - 14. * q],
                r * (0.4 + 2. * q),
                Some(alpha(p.edge, opacity * 0.5)),
                None,
            )
        });
    }
    let base_model = M::rotate_y(pose.base_yaw)
        .then(M::translate(200., 297., pose.base_z))
        .then(around(
            [200., 326.],
            M::rotate_z(super::keys(
                pose.time,
                1550.,
                300.,
                OUT,
                &[(0., 0.), (0.4, 2.), (1., 0.)],
            )),
        ))
        .then(around([200., 297.], M::rotate_x(pose.tilt)))
        .then(root);
    box_paths(
        &mut a,
        192.,
        58.,
        120.,
        base_model,
        base_side(p, pose),
        base_front(p),
        base_top(p),
        p,
        face_light(pose.time, pose.base_yaw, 5450., 800., 0.45),
        pose.base_yaw,
    );
    a.scope(body.then(root), |a| {
        a.rect([182., 256., 36., 18.], 5., p.metal2, Some((2., p.edge)));
        for y in [262., 268.] {
            a.line(&[[188., y], [212., y]], 1.5, p.line);
        }
        a.rect([193., 130., 14., 40.], 4., p.metal2, Some((2., p.edge)));
        for y in [144., 156.] {
            a.line(&[[193., y], [207., y]], 1.5, p.line);
        }
    });
    let torso_model = M::rotate_y(pose.torso_yaw)
        .then(M::translate(200., 214., pose.torso_z))
        .then(body)
        .then(root);
    let torso_front_m = box_paths(
        &mut a,
        64.,
        96.,
        104.,
        torso_model,
        torso_side(p),
        torso_front(p, pose),
        blank(64., 104., p),
        p,
        face_light(pose.time, pose.torso_yaw, 2450., 1200., 0.5),
        pose.torso_yaw,
    );
    let head_model = M::rotate_y(pose.head_yaw)
        .then(M::translate(200., 107. + pose.notice_y, pose.head_z))
        .then(around(
            [200., 130.],
            M::rotate_z(pose.head_tilt + pose.notice_angle),
        ))
        .then(body)
        .then(root);
    let head_front_m = box_paths(
        &mut a,
        50.,
        54.,
        108.,
        head_model,
        head_side(p),
        head_front(p, pose),
        blank(50., 108., p),
        p,
        face_light(pose.time, pose.head_yaw, 2650., 1300., 0.5),
        pose.head_yaw,
    );
    let antenna = head_front_m.project([54., -18., 0.]).unwrap_or([0., 0.]);
    let mut layers = vec![Layer::Vector(VectorLayer {
        paths: a.paths,
        clip: rect,
        opacity: 1.,
    })];
    if pose.torso_yaw == -90. {
        if let Some(origin) = torso_front_m.project([48., 51., 0.]) {
            layers.push(Layer::Text(TextRun {
                text: "CLK-1".into(),
                origin,
                px: 8.5 * scale * pose.camera,
                mono: true,
                weight: 500,
                letter_spacing: scale * pose.camera,
                color: p.paintline,
                ..TextRun::default()
            }));
        }
    }
    let mut word = TextRun {
        text: "Clanky".into(),
        px: 26. * scale,
        weight: 600,
        letter_spacing: -0.52 * scale,
        color: p.ink,
        opacity: pose.word,
        ..TextRun::default()
    };
    let width = measure(&word).width;
    let point = screen
        .project([200., 362. + pose.word_y, 0.])
        .unwrap_or([0., 0.]);
    word.origin = [point[0] - (width + 14. * scale) * 0.5, point[1]];
    let cursor_x = word.origin[0] + width + 4. * scale;
    layers.push(Layer::Text(word));
    let mut cursor = Art::default();
    cursor.rect(
        [cursor_x, point[1] + 7. * scale, 10. * scale, 22. * scale],
        0.,
        alpha(p.eye, pose.word),
        None,
    );
    layers.push(Layer::Vector(VectorLayer {
        paths: cursor.paths,
        clip: rect,
        opacity: 1.,
    }));
    Frame { layers, antenna }
}
