# M11 app interface for the gate and renderer teams

## Pure layout available first

```rust
use clanky::look;
use clanky_core::layout::{Axis, Layout as WorkspaceLayout};
let tree = WorkspaceLayout::branch(Axis::Rows,
    WorkspaceLayout::leaf("A"), WorkspaceLayout::leaf("B"));
let shown = look::layout([1240, 760], 1.0, true, &tree, [10, 20]);
let moved = look::drag_divider(&shown, 0, 300.0)?;
```

`layout(window_px: [u32;2], scale: f64, rails_shown: bool,
workspace_layout: &WorkspaceLayout, cell_px: [u32;2]) -> look::Layout`.
Window and cell sizes are already physical pixels. Only chrome constants scale.

Layout exposes `content`, `top_bar`, optional `left_rail`/`right_rail`, `panes`,
`dividers`, the source `workspace_layout`, and its input metrics. Each pane has
`id`, `rect`, `grid_origin: [f32;2]`, `grid_clip`, `rows` and `cols`.
`PixelRect {x,y,w,h}` uses f32 and has `array() -> [f32;4]` and `contains(x,y)`.
`pane(id)`, `pane_at(x,y)` and `divider_at(x,y)` share the drawn geometry.

`drag_divider(&Layout, divider_index: usize, pointer: f64)` returns
`Result<DragResult,String>` with `ratio`, updated `workspace_layout` and `layout`.
Pointer is physical y for Rows, x for Columns. Divider paths use false for first
child, true for second. Input geometry is never mutated. The app must cancel a
captured drag if its workspace revision changes before applying the result.

Nested minimums preserve 8 columns/3 rows per pane whenever space permits.
`minimum_window_px` supplies a window minimum; `fits_minimum=false` marks a
smaller window. Such windows have bounded geometry, possibly zero-sized grids;
PTY resizing must skip zero-sized grids. Drag refuses an invalid/undersized layout.
`valid` reports numerical metric/ratio validity. Leaf identities are validated by
the existing workspace boundary. Floating-point noise at integer boundaries is
removed before floor, so saving/restoring a dragged ratio does not move a pixel.

## Ratios and persistence

Core `Layout::Split` now carries `ratio: f64` in [0,1]. Deserialization rejects
nonfinite/out-of-range values. Missing ratios default to 0.5; exactly 0.5 is omitted
on serialization, preserving existing half-split JSON and prior gate expectations.
Non-half ratios persist through the existing workspace/layout store, including
its terminal-ID remapping. The existing layout file version remains 1.

`Layout::branch(axis, first, second)` creates a half split.
`set_ratio(path: &[bool], value: f64)` validates before mutation. Cell-space layout
still uses one divider cell; Studio pixel layout uses the design's scaled 10 px gap.
New splits require the minimum grids; tiny resizes do not invent off-window space.

## Motion and config

`ease_out_cubic(from:f32, to:f32, elapsed_ms:f32, duration_ms:f32,
animations:bool)->f32` is clock-free. Disabled/zero-duration motion returns the
target immediately; elapsed time is clamped to the duration.

Config parses opacity, animations, rails, font_family and ui_font_family.
`Config::opacity_alpha()` rounds straight alpha to a byte: default 240, opacity 1
is 255. Defaults are bundled JetBrains Mono and Inter as approved; installed
families remain selectable. Renderer implementation owns font resolution.

Focused check (no GPU or full milestone gate):
`cargo test --offline -p clanky --lib look::tests::studio_geometry_drag_motion_config -j 8`
under the shared build lock. It uses the design's literal geometry/easing/config
values, JSON round-trip and the actual layout-store ID-remapping helper.
[Scene and owner-metadata types](m11-scene-contract.md) define the next delivery.
Scene construction and live window integration follow this interface delivery;
renderer API stubs are not pixel-tested or treated as a working renderer.
