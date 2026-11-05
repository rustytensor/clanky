# M12: the calm look, themes, Post-AI mode

Marc rejected v0.6.1's look on 2026-11-04: buttons did not click, some text was too small, and it felt "odd". The
terminal was sluggish: the window redrew nonstop (fixed on branch m11-idle). He then chose shadcn/ui's structure and
components, in warm stone colours, with this rule: calm, friendly, hand-crafted; never purple, never purple-blue
gradients, no glows. Approved mockups:
- "Clanky, calm" (/srv/devops/build/clanky/mockups/calm.html);
- the component sheets (/srv/devops/build/clanky/mockups/sheets.html), which are the specification for every
  control, in every state.

Where this document and the sheets disagree, the sheets win; tell the lead.

## What changes for the person

- **Layout (shadcn "inset").** A 256 px sidebar on the left: brand, Sessions (window tabs), Agents, Files, and a
  footer with Settings and the masking state. The main area floats beside it as one rounded card, with:
  - a 56 px top bar: sidebar toggle, path, a "Search or run a command" field, and Split and New tab buttons;
  - the panes;
  - a 280 px right column of cards: agent activity, recent events, agent access.
- **Every control works with the mouse.** Sidebar items, buttons, switches (the whole row toggles), block actions
  (Copy, Rerun, Ask agent), Take over, Settings.
- **Type.** Interface: Geist, 14 px; nothing below 12 px anywhere. Terminal: Geist Mono 13 px on 8 × 20 px cells.
- **Colour only with meaning.** Sage = success, brick = failure, clay = an agent is involved. Everything else is warm
  grey. The focused pane gets a ring-coloured border; nothing glows.
- **Not see-through by default.** Opacity defaults to 1.0; the setting stays, for people who want it.
- **Check and cross** are drawn icons, never font glyphs.

## Themes

A theme is a JSON file. Built-in themes ship inside the binary; user themes live in `<config dir>/clanky/themes/`.

```json
{ "name": "Stone", "appearance": "dark",
  "colors": { "sidebar": "#141210", "background": "#1a1816", "card": "#201d1a", "muted": "#26221f",
              "foreground": "#ebe6df", "muted_foreground": "#a0978d", "accent": "#2b2724", "pressed": "#332e2a",
              "border": "#ffefe114", "input": "#ffefe11f", "ring": "#8a8178", "primary": "#ebe6df",
              "primary_foreground": "#1a1816", "sidebar_accent": "#221f1c", "ok": "#93b89a", "err": "#d98b7e",
              "agent": "#d2a476" },
  "ansi": ["#...16 colours..."],
  "window_gradient": null,
  "glow": false }
```

- **Colours** are `#rrggbb` or `#rrggbbaa`. A missing colour is an error, not a default, so a broken theme is
  caught when it loads.
- **`window_gradient`** is either null or `["#top", "#bottom"]`. When set, it fills the sidebar and the window
  background as a vertical gradient.
- **`glow: true`** lets the focused pane and agent-driven panes glow, the way v0.6.1 drew them.
- **Built-in themes:**
  - **Stone** (default, dark and light): the approved values, as on the sheets.
    Marc's approved light contrast correction (Y1, 2026-11-04): `muted_foreground` #6b6259,
    `ok` #456e4d, `err` #a14938, `agent` #8c5925. Their WCAG minimums are 4.55523812,
    4.50939641, 4.55863803 and 4.52410773:1 respectively, including pressed surfaces and
    10% status-badge tints. The requirement is 4.5:1; 4.55 was only a rounding margin.
  - **Paper:** warmer, with softer contrast, dark and light.
  - **Graphite:** cool neutral, shadcn's own neutral greys, dark and light.
  - **Catppuccin** (Marc's yes, 2026-11-04): Latte (light), Frappé, Macchiato and Mocha (dark). MIT. Colours are taken from the upstream
    `catppuccin/palette` palette.json, and the gate checks them against that file. Agent colour = peach.
  - **AI Bedazzled** (dark): the joke theme, every AI cliché in one place.
    - Violet-to-blue `window_gradient` (#7c3aed to #2563eb), glows on, and violet and cyan accents.
    - A drawn sparkle icon beside the brand, and an "AI-powered" badge in the top bar.
    - Its description in Settings: "Every AI cliché, lovingly assembled. For the rare person who wants it."
  - **Unicorn Tears** (light): a soft pastel dream.
    - Pale pink-to-lavender sidebar gradient (#fde2f3 to #e4dcff), with mint, baby-blue and peach accents and
      gentle rounded glows.
    - Text in a deep plum (#3b2a4d), so it still reads at 4.5:1.
    - Settings: "Soft, dreamy, a little sad. Like a unicorn who just watched a sad film."
  - **Unicorn Vomit** (dark): full-saturation rainbow.
    - A red-orange-yellow-green-blue-violet window gradient, a different neon colour on each pane border, glows on,
      and a hot-pink cursor.
    - Settings: "Every colour at once. You were warned."
    - Exempt from the contrast check, like AI Bedazzled; the gate lists the exempt tokens.
  - **Gradients** may have more than two stops: `window_gradient` is a list of 2 to 8 colours, spread evenly.
- **Settings > Appearance:**
  - a theme picker for dark and one for light (`theme_dark`, `theme_light`; `theme` stays dark, light or system);
  - colour pickers on the active theme;
  - "Save as new theme", "Export theme", "Import theme".

  Editing a built-in theme never changes it: it saves a copy.
- **Overrides:** `colors` in the config still overrides any token of the active theme.
- **Contrast:** every built-in theme's text colours keep 4.5:1 or more on their surfaces (WCAG AA), checked by the
  gate. AI Bedazzled is exempt only where its cliché needs it, and the gate lists those tokens.

## Post-AI mode

A plain terminal with tabs and splits, for purists. Settings > General > "Post-AI mode": "A plain terminal with
tabs and splits. No agents, no AI features. For coding and AI purists." Config `post_ai: true`, or start with
`clanky --post-ai`.

- **Gone:**
  - the Agents group and the right column;
  - agent badges, access switches, Take over, Ask agent, compose to an agent, and the pending-change review;
  - the MCP server.
- **Agents are refused at the door.** The server rejects every agent `hello` with the error
  `agents are off (Post-AI mode)`, and `clanky mcp` exits with status 2 and the same message.
- **Kept:**
  - tabs, splits, panes and command cards (shell integration, without Ask agent);
  - search, the palette, Settings, the editor and themes;
  - secret masking, which is a safety feature, not AI.
- **Switching** applies at once. Connected agents are disconnected, which is the same as revoking all agents.

## Clanky the robot (start-up animation)

Marc approved the robot on 2026-11-04 ("I love it"), at about 7 s with mixed pacing: the drama is slow, the
reactions are snappy. The reference animation is /srv/devops/build/clanky/mockups/robot6.html: its shapes, timings
and easings are the spec. Use its "Slow motion" button, or `?t=<ms>` to freeze a frame.

- **The first meeting, about 7 s:**

  | time (s) | what happens |
  |---|---|
  | 0.0–1.6 | He rolls in side-on, looking where he is going (lens barrel forward). Wheels and belt turn; he bobs slightly. |
  | 1.55–1.85 | He brakes snappily with a forward rock, and dust puffs from under the treads. |
  | 1.85–2.45 | Stillness. At 2.15 a quick head twitch: he has noticed you. |
  | 2.45–3.95 | The torso turns 90° to you in 3D, with a slight overshoot (1.2 s). The head follows 200 ms later (1.3 s) with more overshoot. The camera pushes in from 1.0 to 1.07 between 2.4 and 6.0 s. |
  | 3.95–4.9 | The frown settles in over 0.5 s, then holds: lids at 0.72, angled 19°, eyes dark, head tilted 5°. |
  | 4.9–5.3 | He lights up, fast. The lids snap open (220 ms), the eyes flash on (380 ms) and focus, the antenna pings, the charge bars fill 70 ms apart, the glow blooms (900 ms), and his head tilts happily (−7°). |
  | 5.45–6.25 | The tread base turns 90° to you in 3D, the view dipping 20° to show the tread tops. Then he is flat 2D, facing you. |
  | 6.15 | A quick blink (170 ms). |
  | 6.3–7.0 | The wave (560 ms) with an elbow bend and a double claw snap. "Clanky" fades in at 6.5 s, with a blinking cursor in the agent colour. |

- **When it plays:**
  - the full scene, the first time Clanky starts on a machine (a marker file in the state directory);
  - on every later window, a short greeting of about 1.2 s: he is already facing you, lights up, blinks and waves;
  - `clanky +hello` replays the full scene in its own window.
- **Where:** centred in the first pane, drawn over it, while the shell starts underneath.
- **Skipping:** any key or click skips it at once, and that key or click goes to the shell or the control as normal.
  Nothing is ever delayed or swallowed.
- **Turning it off:** config `startup_animation` (default true), or `animations: false`.
- **Colour:** every colour comes from the theme:
  - metal = the muted greys;
  - paint = a theme token `robot_paint` (Stone: dark #6b5236, light #e3c08e), which every built-in theme defines;
  - glowing parts = agent.
- **Renderer:** filled and stroked paths with anti-aliasing for the curves (arms, claws, chest wave, and the
  sidebar's line icons), using the lyon crate (MIT OR Apache-2.0, on the licence list; name it in the commit
  message). Three 3D boxes (base, torso, head) are flat panels through one 4 × 4 perspective matrix (perspective
  700 px); the rest are 2D layers. No 3D engine.
- **Gate (check 15),** with the clock driven by the harness:
  - Full scene: frames only from 0 to 7000 ms, then 0 frames over the next 2 s. The short greeting: frames only
    from 0 to 1200 ms.
  - A key at 300 ms ends the animation within one frame, and the shell receives that key's bytes.
  - At 7000 ms, the antenna light's centre pixel in Stone dark is the agent colour (210, 164, 118, 255),
    tolerance 3.
  - At 5850 ms, the tread base is mid-turn: the pixel at the base's centre-left differs from both the 5400 ms and
    the 6300 ms frames. At 3200 ms, the same holds for the torso against 2400 ms and 4000 ms.
  - The full scene plays once per machine: a second start shows the short greeting.
  - Planted fault `robot` (never stops) must fail the zero-frame check.

## Layout numbers

Logical pixels, multiplied by the scale factor.

| part | size |
|---|---|
| sidebar | 256 px; inside it, 8 px padding |
| inset card | 8 px margin on top, right and bottom; radius 12; 1 px border |
| top bar | 56 px tall, with a 1 px line at its bottom |
| right column | 280 px, with a 1 px line on its left; cards 12 px in from its edges |
| pane area | 12 px inside the inset, between the top bar and the right column |
| gap between panes | 12 px |
| pane | radius 10, header 40 px, content padding 10 px top and bottom, 12 px left and right |
| grid origin | (pane.x + 12, pane.y + 50) |
| columns | floor((pane.w − 24) / cell_w) |
| rows | floor((pane.h − 60) / cell_h) |

Splits and the divider drag work as in M11, with the 12 px gap.

## Gate: `scripts/gate-m12-calm.sh`

Fixed state: 1240 × 760 physical pixels, scale 1; bundled fonts; terminal Geist Mono 13 px with 8 × 20 cells;
theme Stone dark; opacity 1.0. Two terminals in one tab, split top and bottom at ratio one half. Pane A (top) is not
focused and has agents off. Pane B is focused and shows one finished block, `cargo build`, exit 0. Sidebar and right
column are shown. Every expected value below was worked out from the numbers above before any code.

1. **Layout (exact).**
   - Inset (256, 8, 976, 744). Pane area x 268..940, y 76..740.
   - Pane A = (268, 76, 672, 326); pane B = (268, 414, 672, 326).
   - Grid A origin (280, 126); grid B origin (280, 464). 81 columns × 13 rows each; the PTYs report the same.
2. **Sidebar and right column hidden (Ctrl+Shift+B).**
   - Inset (8, 8, 1224, 744). Pane A = (20, 76, 1200, 326): 147 columns × 13 rows.
3. **Divider drag (exact).** Drag the gap to y 300.
   - Pane A = (268, 76, 672, 218), 7 rows.
   - Pane B = (268, 306, 672, 434), 18 rows.
   - Save and restore give the same rectangles.
4. **Pixels, Stone dark** (premultiplied RGBA, tolerance 3):

   | point | what | colour |
   |---|---|---|
   | (128, 700) | sidebar | (20, 18, 16, 255) |
   | (257, 9) | outside the inset's rounded corner | (20, 18, 16, 255) |
   | (264, 16) | inside the inset | (26, 24, 22, 255) |
   | (600, 63) | top-bar line (8 % warm white over the background) | (44, 41, 38, 255) |
   | (939, 250) | pane A border | (44, 41, 38, 255) |
   | (939, 600) | pane B border, focused (ring) | (138, 129, 120, 255) |
   | (930, 390) | inside pane A | (26, 24, 22, 255) |
   | (968, 200) | right-column card | (32, 29, 26, 255) |
5. **Pixels, Stone light,** same points:

   | point | colour |
   |---|---|
   | (128, 700) | (243, 240, 235, 255) |
   | (257, 9) | (243, 240, 235, 255) |
   | (264, 16) | (251, 250, 248, 255) |
   | (600, 63) | (231, 225, 217, 255) |
   | (939, 250) | (231, 225, 217, 255) |
   | (939, 600) | (163, 154, 143, 255) |
   | (930, 390) | (251, 250, 248, 255) |
   | (968, 200) | (255, 255, 255, 255) |
6. **AI Bedazzled.** The vertical gradient is sampled at pixel centres, t = (y + 0.5) / 760:
   - (128, 20) = (122, 59, 237, 255);
   - (128, 740) = (39, 98, 235, 255).
7. **Text floor.** The fixed scene contains no UI text run below 12 px. The scene builder's runs are listed and
   checked.
8. **Clicks, through the app's real input path** (pointer events into the same handler the window uses, not into
   the scene):
   - Click New tab: the tab count goes from 1 to 2, and the sidebar shows 2 sessions.
   - Click Split: pane B splits; there are 3 panes.
   - Click the "~/git/clanky" access row (the text, not the switch): agent access for pane A turns on, and v3 status
     reports it.
   - Click the second session in the sidebar: the focused tab changes to it.
   - Hover pane B's block: its actions become visible. Click Copy: the clipboard holds the block's masked output
     text exactly.
   - Click Settings in the sidebar: the Settings panel opens; Escape closes it.
   - A disabled control ignores clicks.
9. **Idle.** After start-up, 2 seconds with no input draw 0 frames. One key press draws exactly the frames its echo
   needs (at most 2).
10. **Themes.**
    - Every built-in theme loads.
    - A theme missing one colour is refused with the loader's message.
    - The Catppuccin colours equal upstream palette.json, version pinned in the gate.
    - Every text colour of every built-in theme except AI Bedazzled and Unicorn Vomit is at least 4.5:1 on its surface.
    - Save as new, then export, then import gives an identical file.
11. **Post-AI mode.**
    - `post_ai: true` lays out with no right column: pane A = (268, 76, 952, 326), 116 columns.
    - An agent hello is refused with `agents are off (Post-AI mode)`.
    - `clanky mcp` exits 2 with that message.
    - The owner API still works.
12. **Planted faults:**
    - `CLANKY_GATE_FAULT=look` (square inset corners) must fail at (257, 9);
    - `click` (drop pointer routing) must fail check 8;
    - `idle` (continuous redraw) must fail check 9.
13. **Regression.** `scripts/gate-rc.sh`, with this gate added to it.
14. **Pictures for Marc:** `$CARGO_TARGET_DIR/look/` gets one PNG per built-in theme. The gate respects the caller's
    `CARGO_TARGET_DIR`; it no longer hard-codes a team's folder.

## Fonts

Bundle Geist and Geist Mono v1.9.1 (Regular, Medium, SemiBold; OFL-1.1, from github.com/vercel/geist-font,
release v1.9.1), plus OFL-Geist.txt. Marc approved Geist on 2026-11-04.
- JetBrains Mono and Inter stay bundled, because older configs may name them.
- The M1/M2 fixtures keep DejaVu Sans Mono.
- The packaging gate's expected file list gains the new font and licence files. That is an expected-value
  correction, in its own commit, with the release as evidence.

## Work split

- **clanky-editor:**
  - the app: layout, every component exactly as on the sheets, the sidebar and its data, the top bar, the right
    column;
  - Post-AI mode, and pointer routing for every control;
  - docs;
  - built on m11-idle, the redraw fix.
- **clanky-build:**
  - Clanky the robot: lyon paths, the perspective panels, the animation timeline, `clanky +hello`;
  - the theme system: format, loader, built-ins (Stone, Paper, Graphite, Catppuccin, AI Bedazzled, Unicorn Tears, Unicorn Vomit), user theme
    folder, and the Settings data for the picker, save, export and import;
  - the Geist fonts in the renderer.
- **clanky-pkg:** this gate, the click harness, the idle check, the theme checks, the packaging list, and the
  gate's CARGO_TARGET_DIR fix.

The lead reviews every component against the sheets at 2× crops before Marc sees anything.
