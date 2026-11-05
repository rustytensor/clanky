# Theme gate commands

**Current result: contrast FAIL; storage PASS.** Stone light has 15 failing pairs,
minimum 3.8150:1 for muted_foreground on pressed. The other 6 own themes pass;
6 upstream/joke themes are explicitly exempt. Both planted faults were caught.
No colours, thresholds or approved expectations were changed.

`CARGO_TARGET_DIR=/srv/devops/build/clanky-m12-skins/target scripts/gate-m12-themes.sh`

The caller may supply its own target directory and `CLANKY_BUILD_LOCK`. Both checks
run even if contrast fails. Exit 0 means both passed; exit 1 means a check failed.
Use `--contrast-only` or `--storage-only` to run one part. No GPU, UI or full gate.

## Expectations fixed before running

Contrast: WCAG 2.x sRGB relative luminance and `(lighter + .05)/(darker + .05)`.
Every checked pair must be **at least 4.5:1**, before rounding; no tolerance below it.
Reference: https://www.w3.org/TR/WCAG22/#dfn-contrast-ratio

- Foreground/secondary text: sidebar, background, card, muted, accent, pressed and sidebar_accent.
- Success/error/agent text: background, card and muted, both plain and with the component's 10% badge tint.
- Primary labels: primary, primary_pressed and the 10%-toward-background hover fill.
- Reverse cursor: background text on foreground fill.
- Sidebar gradient: foreground/secondary text across each continuous segment, including interior luminance minima.

Colours are composited before comparison, at window opacity 1. Disabled controls,
nontext decoration and program-selected ANSI colours are outside this text-token check.
The built-in loader supplies the theme list; the script does not guess it from filenames.

Explicit full-theme exemptions (printed with exempt text tokens): AI Bedazzled,
Unicorn Vomit, Catppuccin Latte/Frappé/Macchiato/Mocha. No prefix-based exemptions.
Stone, Paper, Graphite and Unicorn Tears are not exempt.

Storage: save a copy of Stone dark named Gate Custom, background #010203. Expect
(1,2,3,255) after reopening; Stone remains (26,24,22,255). Save/export/import/re-export
must produce byte-identical files. Save/import to an existing theme and export to an
existing sentinel file must fail without changing their bytes. Scratch files live
under CARGO_TARGET_DIR and are removed, including after a failed check.

## Prove the checks fail

- `CLANKY_THEME_GATE_FAULT=contrast scripts/gate-m12-themes.sh --contrast-only`
  replaces Stone dark foreground with its background in memory. It must report
  `stone-dark foreground/background: 1.0000:1`, independently of Stone light's existing failures.
- `CLANKY_THEME_GATE_FAULT=storage scripts/gate-m12-themes.sh --storage-only`
  appends a newline to the exported file. It must report differing export bytes.

Known before running: the README records Stone light contrast below 4.5:1. This gate
must report those failures, not change approved colours or invent another exemption.
