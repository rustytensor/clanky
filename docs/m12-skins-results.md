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
