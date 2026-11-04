# Scene rendering

`Renderer::draw_scene(&Scene)` paints layers in order. All positions, sizes,
font sizes, borders and blur radii are physical pixels. Input colours are straight
RGBA; blending and `offscreen_scene` readback are premultiplied RGBA8.

- Shapes: rounded-rectangle distance fields, one-pixel edge coverage, vertical
  gradients, inset borders and Gaussian-like outside shadows fading to zero at
  the blur radius. No blur framebuffer passes.
- Grids: origin and clip are window-relative; underlines are local row/column
  ranges with exclusive end columns. Default black backgrounds are transparent
  in scenes so card fills show through; other backgrounds are opaque. Images
  use nearest sampling and cannot escape the pane clip.
- Text: origin is the top-left of the line box. Cosmic-text shapes complete UI
  runs with explicit newlines, without automatic wrapping. Glyph coverage uses
  linear sampling; text and images have separate blend-safe shaders.

`GridLayer::opacity` and `TextRun::opacity` are finite values from 0 to 1.
Use 1 for the previous output. The factor multiplies premultiplied RGB and alpha
for each primitive, including grid backgrounds, glyphs, underlines, cursor and
images. This is a per-primitive fade, not an offscreen group compositing pass.
`scene::tests::scene_opacity` checks literal pixel values at 0, 0.5 and 1;
`CLANKY_OPACITY_FAULT=1` plants a full-opacity fault in its half-opacity sample.

Scene defaults are bundled JetBrains Mono and Inter. `set_font_families(terminal,
ui)` accepts bundled or installed families; default-family host duplicates are
excluded so shipped faces win. Window metrics follow `set_metrics`. Offscreen
scenes use bundled fonts only and fixed 16 px / 10x20 cells.

`measure_text(&TextRun)` is GPU-free and deterministic with the bundled fonts.
It reports the widest line's advance width (not ink bounds), maximum font ascent
and descent, and baseline spacing `px * 1.25`, all in physical pixels.
`Renderer::measure_text` uses the window's configured families instead. Both use
the same cosmic-text shaping code as drawing. Empty or invalid runs measure zero.
`TextRun::default()` sets `letter_spacing` to 0 and opacity to 1. Tracking uses
cosmic-text's shaped glyph advances; measurement excludes the final tracking gap.
Spacing must be finite and within -512..=512 px. Ligatures and combining clusters
remain cosmic-text's responsibility; text is never split into separately shaped chars.
The focused `scene::tests::text_metrics` check compares `Clanky` at 20 px with
independent cosmic-text advances plus five 1.5 px gaps. `CLANKY_TEXT_METRICS_FAULT=1`
removes tracking from the measured run and must fail that check.

`supports_transparency()` reports premultiplied surface support. On an opaque
fallback, scene clear alpha is ignored; the app should report that once and must
request a transparent window when creating it. For an animated cursor drawn by
the app, clear the grid snapshot's cursor to avoid the static underline cursor.

Legacy `draw`, `draw_with_underlines` and offscreen fixtures keep their original
pipeline and DejaVu font. The focused check is `scene::tests::scene_pixels` in the
library tests: fixed design pixels, alpha/gradient/image clipping, bundled glyph,
underlines and the original M1/M2 samples. `CLANKY_SCENE_FAULT=corner` deliberately
fails its rounded-corner assertion. Run with software Vulkan for headless checks.

Limits are explicit errors: 128 MiB of distinct visible image pixels, 64 MiB of
vertices, 16 KiB per UI text run, 1 MiB of UI text per scene and 65,536 layers.
Glyph-atlas exhaustion retries preparation once with a fresh atlas; there is no
mid-frame reuse of old atlas positions. The full app look gate and compositor/
installed-font acceptance belong to integration, not this component check.
