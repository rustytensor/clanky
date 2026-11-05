# M12 fonts and themes

## Font check

Command: `cargo test -j 8 -p clanky-render --lib geist_measurements -- --nocapture`.
Run with `CLANKY_FONT_FAULT=1` first: the deliberately wrong width must fail.

Expected widths from independent `hb-shape` on the v1.9.1 TTFs:
`0000000000` in Geist at 14 px is 94.08, 95.48 and 96.88 px for weights 400, 500 and 600.
The first draft used hmtx advances alone (92.82, 94.22, 95.62); its check failed because
Geist adds 10 font units of kerning to each of the first nine zeros. HarfBuzz confirms
those adjustments independently of cosmic-text. The corrected check includes them.
In Geist Mono at 13 px, all three are 78 px. Tolerance: 0.02 px for floating-point shaping.
The grid positions remain fixed at 8 × 20 px; glyph advances do not determine cell width.
The legacy M1/M2 DejaVu renderer is unchanged.
Result: planted wrong width failed; corrected independent widths passed (all six faces).
No GPU check or visual crop comparison was run for this font-only part.

Font source: https://github.com/vercel/geist-font/releases/tag/1.9.1 (OFL-1.1).
The six TTFs and `assets/fonts/OFL-Geist.txt` are copied without modification.
Older JetBrains Mono and Inter families remain bundled.

## Theme check

Command: `cargo test -j 8 -p clanky --lib themes::tests::builtin_documents -- --exact --nocapture`.
Run with `CLANKY_BUILTIN_FAULT=1` first: black substituted for Catppuccin peach must fail.
Expected: 13 loadable documents, complete 16-slot ANSI palettes; Catppuccin matches
upstream v1.8.0 commit `07d02aa110ef9eb7e7427afca5c73ba9cf7f8ebd` using the mapping in
`assets/themes/README.md`. The three joke gradients, glow flags and Tears foreground
must match the design literals. No theme format change.

Result: planted black-agent fault failed on Latte; the normal check passed all 13 themes.
A separate WCAG calculation over foreground, secondary and semantic text on the seven
normal/hover/pressed surfaces, plus primary labels on both button fills, found minimums:
Paper dark 5.87, Paper light 4.92, Graphite dark 5.23, Graphite light 6.11, Tears 6.15.

Lead decision: keep exact upstream Catppuccin colours as the theme's identity.
The 4.5:1 requirement applies to our own themes only, not Catppuccin.
Latte peach/green remain 2.64:1 and 2.96:1 on base; no colour values were changed.
The app's theme-specific sparkle/badge, rainbow borders and cursor remain app-team work.

## Settings theme-data gate

Command: `cargo test -j 8 -p clanky --lib theme_store_round_trip -- --nocapture`.
First run with `CLANKY_THEME_STORE_FAULT=1`: a deliberately corrupted exported file must fail.
Real input: Stone dark JSON renamed Workshop, background changed to #010203.
Expected before running: preview/background (1,2,3,255), agent (210,164,118,255);
export and import bytes identical to that input. An external edit to #040506 reloads to
(4,5,6,255); malformed JSON keeps those last-good values; deletion removes the picker entry.
A saved copy with agent (10,20,30,255) never changes Stone. Existing destinations and
path traversal must be refused without overwriting any bytes.
Config post_ai=true and CLI --post-ai both set the flag; a string boolean is refused.
The agent override #10203080 resolves to (16,32,48,128), without modifying the stored theme.
Result: the planted export corruption failed; the normal gate passed on its first run.
A focused follow-up `cargo test -j 8 -p clanky --lib recreated_theme_clears_old_error`
also passed: deleting an invalid file and recreating it through save_new clears its old
error immediately, including when the next reload sees unchanged bytes.
No full suite, GPU check or executable Post-AI enforcement check was run.
