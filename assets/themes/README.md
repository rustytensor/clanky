# Theme API

The app imports `clanky::themes::{Theme, Colors, Appearance, builtin, BUILTINS}`.

- `builtin("stone-dark")` and `builtin("stone-light")` return independent theme values.
- `BUILTINS` supplies stable picker IDs, names and appearances; `default_id` selects Stone.
- `Theme::from_json` rejects missing tokens, unknown fields, malformed colours and invalid gradients.
- `theme.colors.background` and the other 18 fields are straight `[u8; 4]` values.
- `theme.ansi` holds 16 straight RGBA colours in ANSI order. Truecolour terminal data is not a theme token.
- `window_gradient` is `Option<Vec<Color>>`: 2–8 evenly spaced stops, or none. `glow` is explicit.
- `with_overrides` returns a changed copy without altering a built-in. `to_json` writes canonical JSON.
- `clanky::theme_store::ThemeStore` supplies user-folder loading/reload, picker previews, save-as-new,
  byte-preserving export/import and per-file errors. See [the Settings API](../../docs/theme-settings-api.md).
  The app team owns UI integration and replacing the M11 `look::Theme`.

Stone UI values come from the approved component sheets. The sheets' dark border/input base is #fff0e1,
not the design example's #ffefe1. `robot_paint` is required: Stone dark #6b5236 and light #e3c08e. Alpha is rounded to RGBA8 (20 and 31). `primary_pressed` is required too: Stone dark #d6d0c8, light #3d3832. Other built-ins move primary about 8% toward background.
ANSI colours are a warm palette
chosen for readable terminal text; they are not fixed by the mockup.

Marc approved the Stone light contrast correction (Y1, 2026-11-04, relayed by the lead):
`muted_foreground` #6b6259, `ok` #456e4d, `err` #a14938, `agent` #8c5925.
Matching Stone ANSI entries use the corrected colours too; other themes are unchanged.
The earlier gate found 15 failing pairs, minimum 3.8150:1 at muted_foreground/pressed.
The corrected WCAG minima are 4.55523812, 4.50939641, 4.55863803 and 4.52410773:1,
respectively. Secondary text is checked on pressed; the three status minima include
10% badge tints over muted (alpha 26/255). The required threshold is 4.5:1, not 4.55:1.
Verification: `gate-m12-themes.sh --contrast-only` (unchanged gate from 01168ab) passed
all 7 own themes with 0 failing pairs; the 6 approved exemptions remain explicit.

## Remaining built-ins

Stable IDs: `paper-dark`, `paper-light`, `graphite-dark`, `graphite-light`, `ink`,
`catppuccin-latte`, `catppuccin-frappe`, `catppuccin-macchiato`, `catppuccin-mocha`,
`ai-bedazzled`, `unicorn-tears`, `unicorn-vomit`. Stone remains the default.
Paper uses warm parchment colours. Graphite uses shadcn's neutral grey scale;
secondary text uses neutral 400 dark / 600 light to remain readable on pressed surfaces.
The three joke themes alone enable gradients and glow. Their descriptions and gradient
stops are the design's literals; Unicorn Tears uses deep-plum text and pastel surfaces.

Catppuccin source: `catppuccin/palette`, version 1.8.0, commit
`07d02aa110ef9eb7e7427afca5c73ba9cf7f8ebd`.
`upstream/catppuccin-palette.json` is the unmodified upstream file, SHA-256
`4bc114bb6b3c9a9c9e156564aa84625aef32c5da514d9dd431cf1fcad433a05f`.
MIT licence: `upstream/LICENSE-Catppuccin`. Upstream URL:
https://github.com/catppuccin/palette/blob/07d02aa110ef9eb7e7427afca5c73ba9cf7f8ebd/palette.json

Mapping (all four flavours): sidebar=crust, background=base, card=mantle,
muted/accent/sidebar_accent=surface0, pressed/border=surface1, input=surface2,
foreground/primary=text, muted_foreground/primary_pressed=subtext1,
primary_foreground=base, ring=overlay1, ok=green, err=red, agent/robot_paint=peach.
The 16 ANSI slots are upstream `ansiColors` by `code`, including distinct bright colours.
No mauve/lavender UI accents, gradients or glows. Catppuccin uses upstream subtext1 for
primary_pressed rather than mixing a new colour, to preserve exact upstream values.

**Contrast policy (lead decision):** preserve exact upstream Catppuccin colours;
they define the theme's identity. The 4.5:1 rule applies to our own themes only,
not Catppuccin. For reference, Latte peach on base is 2.64:1 and green is 2.96:1.
The app team owns the joke themes' sparkle/badge, rainbow pane borders and hot-pink
cursor; these have no separate fields in the approved theme format.

Focused remaining-builtins check:
`cargo test -p clanky --lib themes::tests::builtin_documents -- --exact`.
It loads all 13 documents, round-trips every ANSI slot, compares all four Catppuccin
palettes to the upstream file, and checks the three literal gradients and glow flags.
`CLANKY_BUILTIN_FAULT=1` changes Catppuccin's agent colour to black and must fail.

Focused Stone check: `cargo test -p clanky --lib themes::tests::stone_documents -- --exact`.
It compares all 19 Stone tokens with literal sheet channels, refuses a missing ring, and preserves a literal
six-stop gradient. `CLANKY_THEME_FAULT=1` substitutes black for Stone's sidebar and must fail.

## Darker dark themes (Marc approved, 2026-11-12)

The dark Stone, Paper and Graphite backgrounds are about 30% darker (#141210, #1f1c17, #111111) and their
terminal colours are richer; the old shared ANSI set was too pale. Ink is new: neutral near-black (#121212)
with blue primary buttons. Unicorn Vomit and AI Bedazzled keep their gradients in deeper tones so sidebar text
reads on every band, and lose their contrast exemption. `scripts/gate-t1-themes.sh` checks every own theme:
interface text 4.5:1, ANSI 1-7 and 9-15 4.5:1 on the background, ANSI 8 3:1. Catppuccin stays upstream.
