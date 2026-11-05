# Theme API

The app imports `clanky::themes::{Theme, Colors, Appearance, builtin, BUILTINS}`.

- `builtin("stone-dark")` and `builtin("stone-light")` return independent theme values.
- `BUILTINS` supplies stable picker IDs, names and appearances; `default_id` selects Stone.
- `Theme::from_json` rejects missing tokens, unknown fields, malformed colours and invalid gradients.
- `theme.colors.background` and the other 17 fields are straight `[u8; 4]` values.
- `theme.ansi` holds 16 straight RGBA colours in ANSI order. Truecolour terminal data is not a theme token.
- `window_gradient` is `Option<Vec<Color>>`: 2–8 evenly spaced stops, or none. `glow` is explicit.
- `with_overrides` returns a changed copy without altering a built-in. `to_json` writes canonical JSON.
- User-folder storage and import/export will follow this first API commit. This module does not yet replace
  the M11 `look::Theme`; the app team owns that integration.

Stone UI values come from the approved component sheets. The sheets' dark border/input base is #fff0e1,
not the design example's #ffefe1. `robot_paint` is required: Stone dark #6b5236 and light #e3c08e. Alpha is rounded to RGBA8 (20 and 31). ANSI colours are a warm palette
chosen for readable terminal text; they are not fixed by the mockup.

Known spec conflict, reported to the lead: Stone light secondary text on `muted` is 4.40:1; success is 4.36:1
and agent is 4.45:1. The approved colours are preserved, not silently darkened to satisfy the 4.5:1 gate.

Focused check: `cargo test -p clanky --lib themes::tests::stone_documents -- --exact`.
It compares all 18 Stone tokens with literal sheet channels, refuses a missing ring, and preserves a literal
six-stop gradient. `CLANKY_THEME_FAULT=1` substitutes black for Stone's sidebar and must fail.
