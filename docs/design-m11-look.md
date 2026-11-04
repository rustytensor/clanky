# M11: the look (Studio layout with a glass finish)

Marc chose the "Studio" direction with the "Glass" finish (mockups: three look directions, 2026-11-04).
Clanky must look good enough that developers want it on sight, and work better with the mouse.
This milestone comes before the 1.0 release candidate.

## What changes for the person

- **Window chrome.** A top bar (brand mark, a search field that opens the palette, pills for server uptime and
  masking), a left rail (Sessions = window tab groups, Agents = connected agent peers with run/idle state,
  Files = open editor files with pending-change counts) and a right rail (agent activity sparkline, a timeline of
  the last 20 events, and one access switch per terminal pane). Ctrl+Shift+B folds both rails away; the top bar
  then shows compact tab pills.
- **Panes are floating cards.** Rounded corners, a soft shadow and a 10 px gap between panes. The focused pane has
  a blue edge and glow, a pane an agent is driving has a violet edge and glow. Each pane has a header line: its
  title (working folder or running command), who drives it, and an access chip.
- **Command blocks are cards.** Each OSC 133 block is drawn on a card; its command row carries an exit chip
  (green ✓ 0 / red ✗ N) and the duration. A running block has a violet edge and a pulsing dot.
- **Glass.** The window background is translucent (opacity 0.94 by default). GNOME on Wayland gives no blur of
  what is behind a window, so this is translucency only.
- **Motion.** The cursor glides (80 ms), panels fade and rise in (120 ms), scrolling is smooth. `animations: false`
  turns all motion off.
- **Mouse.** Drag the gap between two panes to resize them. Click a session, file, agent or access switch in the
  rails. Click the search field to open the palette.
- **Panels restyled.** Palette, search, compose, editor review controls and file prompts are rounded cards with
  shadows in the UI font. Compose docks at the bottom of its target pane instead of floating over the window.

## Renderer: a scene, not one grid

Today the app composes every pane into one large cell snapshot. M11 adds a scene renderer in `clanky-render`:

```rust
pub struct Scene {
    pub clear: [u8; 4],              // straight RGBA; the target stores premultiplied colour
    pub layers: Vec<Layer>,          // drawn in order
}
pub enum Layer {
    Shape(Shape),                    // rounded rectangle
    Grid(GridLayer),                 // a terminal snapshot at a pixel origin, clipped
    Text(TextRun),                   // UI text in the UI font
}
pub struct Shape {
    pub rect: [f32; 4],              // x, y, w, h in physical pixels
    pub radius: f32,
    pub fill: Fill,                  // Solid([u8;4]) or Vertical([u8;4],[u8;4]) gradient
    pub border: Option<(f32, [u8; 4])>,
    pub shadow: Option<Shadow>,      // offset, blur radius, spread, colour; drawn outside the shape
}
pub struct GridLayer { pub origin: [f32; 2], pub clip: [f32; 4], pub snapshot: TerminalSnapshot,
                       pub underlines: Vec<Underline> }
pub struct TextRun { pub origin: [f32; 2], pub text: String, pub px: f32, pub color: [u8; 4],
                     pub weight: u16, pub mono: bool }
```

- Shapes use a signed-distance rounded-rectangle shader with 1 px anti-aliasing. Shadows and glows use the
  same distance, faded with a Gaussian-like falloff over the blur radius. No offscreen blur passes.
- Colours blend premultiplied. The surface uses a premultiplied alpha mode when the adapter offers one, else
  opaque (then `opacity` is ignored and the status line says so once).
- The existing `draw(snapshot)` path and every `offscreen_*` fixture stay byte-for-byte as they are; M1 and M2
  pixel gates do not change.
- Fonts: bundled JetBrains Mono (terminal) and Inter (interface), OFL-1.1, approved by Marc 2026-11-04. Config can
  name other installed families, loaded through cosmic-text's font database (no new crate). The gate uses only
  bundled fonts, so results do not depend on the machine. The existing M1/M2 offscreen fixtures keep DejaVu Sans
  Mono so their pixels do not change.
- `offscreen_scene(scene, width, height) -> Vec<u8>` reads back premultiplied RGBA for the gate.

## Layout: pixels from the core layout tree

The core layout tree stays the source of truth for which terminals exist and how they split. It gains a split
ratio (default one half; clamped so each side keeps at least 3 rows or 8 columns), saved with the workspace and
in the layout store. The app maps it to pixels:

Constants in logical pixels, multiplied by the scale factor: top bar 40, left rail 210, right rail 270, outer gap
10, gap between panes 10, pane corner radius 11, pane padding 12, pane header 24, block card radius 8, card
inset 4.

- Content area = window minus top bar and the shown rails, then minus the outer gap on every side.
- A split divides the content extent minus one gap. The first child gets floor(available × ratio); the second
  gets the rest. With ratio one half this is floor((extent − 10) / 2).
- Grid origin = (pane.x + 12, pane.y + 12 + 24). Columns = floor((pane.w − 24) / cell_w);
  rows = floor((pane.h − 48) / cell_h). PTY sizes follow these numbers.
- Dragging a gap sets the ratio so the gap's centre follows the pointer.

## Theme tokens (fixed for the gate)

| token | colour | use |
|---|---|---|
| window | #0f1522 at the configured opacity | window background |
| rail | #0b111c | top bar and both rails |
| panel | #131b2b | pane cards |
| line | #1f2a3f | dividers, card borders |
| ink | #dbe4f3 | text |
| dim | #7685a3 | secondary text |
| accent | #ff9f5a | prompt mark, cursor, active rail item |
| violet | #a78bfa | agent edge, agent glow, masked text |
| teal | #4fd1c5 | hashes, secondary accent |
| ok | #5fd38d | exit 0 |
| err | #ff6b7a | non-zero exit |
| focus | #8fb4ff | focused pane edge and glow |

Block card fill: white at 3 % over panel; the card spans the block's rows plus the 4 px card inset on every side.
Exit chip: a pill on the command row, from 2 px below the row's top to 2 px above its bottom, its right edge 6 px
inside the card, 8 px padding around its text, filled ok or err at 18 % over the card.
Only the focused pane and agent-driven panes get a shadow or glow; other panes have none.

## Config (new keys)

`opacity` (0.5 to 1.0, default 0.94), `animations` (default true), `rails` (default true),
`font_family` (terminal font family, default bundled JetBrains Mono), `ui_font_family` (default bundled Inter).
Unknown keys stay errors.

## Gate: `scripts/gate-m11-look.sh`

One command, PASS or FAIL. The harness binary `clanky-look-gate` builds the real app layout and scene from fixed
state, renders it offscreen at 1240 × 760 physical pixels, scale 1, bundled font at 16 px (cells 10 × 20), with the
software Vulkan driver, and prints observations. Python compares them with the values below, which were worked
out from the constants above before any code was written.

Fixed state: two terminals in one window tab, split top/bottom (ratio one half). Pane A on top is not focused and
has agents off. Pane B below is focused. Pane B's grid shows one finished block `cargo build` with exit 0 on row 0.
Rails shown. Opacity 0.94 (alpha 240).

1. **Layout (exact).** Content area x 220..960, y 50..750. Pane A = (220, 50, 740, 345). Pane B = (220, 405, 740, 345).
   Grid A origin (232, 86), 71 columns × 14 rows. Grid B origin (232, 441), 71 × 14. The PTYs report 14 rows and
   71 columns.
2. **Rails hidden (exact).** After Ctrl+Shift+B: pane A = (10, 50, 1220, 345), 119 columns × 14 rows;
   pane B = (10, 405, 1220, 345).
3. **Divider drag (exact).** Rails shown. Drag the gap from y 400 to y 300: pane A = (220, 50, 740, 245), 9 rows;
   pane B = (220, 305, 740, 445), 19 rows. Saving and restoring the layout gives the same rectangles.
4. **Pixels (premultiplied RGBA, tolerance 3 per channel unless stated).**
   - Gap above pane A, (590, 44): window colour at alpha 240 = (14, 20, 32, 240).
   - Inside pane A's bottom-right padding, (950, 380): panel = (19, 27, 43, 255).
   - Rounded top-left corner of pane A, (221, 51): window colour (14, 20, 32, 240); (226, 56): panel (19, 27, 43, 255).
   - Left rail, (100, 700), and right rail, (1100, 700): rail = (11, 17, 28, 255).
   - Block card in pane B, (230, 451): card = (26, 34, 49, 255).
   - Exit chip in pane B, (936, 451): chip = (38, 66, 66, 255).
   - Focus glow, 4 px left of pane B, (215, 577): blue channel at least 8 above the window colour's 32, and above
     red. (A hue check, because the falloff shape is the renderer's choice.)
5. **Agent glow.** Same state with agent access on and an agent command running in pane A: at (215, 222),
   red and blue each at least 20 above green.
6. **Motion (exact, pure functions).** Ease-out cubic, p(t) = 1 − (1 − t/d)³. Cursor from x 100 to 200 over
   80 ms: 100 at 0 ms, 187.5 at 40 ms, 200 at 80 ms and 120 ms. Panel opacity over 120 ms: 0.875 at 60 ms.
   With `animations: false`: 200 and 1.0 at 0 ms.
7. **Config.** `{"opacity":0.3}` and `{"rails":"yes"}` are configuration errors; `{"opacity":1}` gives alpha 255 at
   (590, 44).
8. **Planted fault.** `CLANKY_GATE_FAULT=look` renders pane corners with radius 0 in the harness only. The gate
   must fail at the rounded-corner check, (221, 51): expected window colour, observed panel colour.
9. **Regression.** The gate then runs `scripts/gate-rc.sh` once (all earlier gates, unchanged expected values).
10. **Picture for review.** The gate writes the rendered scene to `$CARGO_TARGET_DIR/look/scene.png`. Marc judges
    the look from it and from the running app; that judgement is the acceptance, not the pixel checks.

## Work split

- Renderer (clanky-render): scene types, shape pipeline, premultiplied blending and transparent surface,
  grid layers at pixel origins with clipping, UI text runs, `offscreen_scene`. Commit the types first, as an
  API-only commit, so the app work can start against them.
- App (clanky, clanky-core layout ratio, layout store): pixel layout, rails and their data, pane headers, block
  cards, glows, compose dock, restyled panels, motion, divider drag, new config keys, docs and docs gate.
- Gate (scripts, `clanky-look-gate`): this gate, added to `gate-rc.sh`.

No new third-party crate. No change to the socket API shapes; any data the rails need that the owner cannot read
today is added as owner-only v3 operations, documented and covered by the docs gate.
