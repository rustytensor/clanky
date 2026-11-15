# Remaining M12 Settings click checks

Command: `CARGO_TARGET_DIR=<target> scripts/check-settings-clicks.sh`.
The script takes the shared build lock. Set `CLANKY_GATE_BINARIES_READY=1` for matching prebuilt binaries;
`CLANKY_GATE_BIN_DIR` can select captured binaries. Call outside an outer lock.
Uses private Xvfb, xdotool, Pillow capture and a private server/home/config. No desktop input is sent.

Scope split with clanky-pkg, checked against its `364ab69`: its script owns access-row permission,
Settings open/Escape/close, Light mode, stepper plus, animations, disabled controls and block-hover work.
This script uses opening/closing as setup, without repeating those acceptance checks.

## Expectations fixed before the run

Quiet layout (2026-11-14, Marc's Y1 of the redesign): Settings opens from the title bar's gear (1217,19);
its frame, nav and pages keep their places. The theme cards and mode segments were replaced by the theme picker
and "Follow the system"; the Side columns row is the Tools panel row; the sidebar toggle is the title bar's tools
toggle. Places come from the approved quiet mockup's CSS and the Q1 gate; words are found on screen as the Q1
gate finds them. Rows changed for this are marked by their new control names below.

Sources: approved `mockups/settings.html`, independent Chrome bounds with bundled Geist fonts,
and check 8 of `docs/design-m12-calm.md`. Window 1240×760, scale 1, opacity 1.
Panel 780×600, centred at (230,80). RGB tolerance is 3 per channel.

| Remaining control | Click point | Expected result |
|---|---|---|
| Each of General, Appearance, Terminal, Editor, Agents, History, Keys | the nav name, found by its word | At a point right of the longest nav name on each name's line: the open page's row is the theme's sidebar_accent, every other row its sidebar (Stone: (26,24,21) and (15,13,11)) |
| Theme picker row: Stone in the Dark (left) column | found by its word in the left half of the Appearance body, scrolling if needed | theme_dark becomes stone-dark; body pixel (997,600) changes from (1,2,3) to Stone's background (20,18,16) |
| Follow the system On / Off | its On / Off words on the Follow row | Config theme becomes system, then dark or light (the system's mode); body pixel (997,600) is that theme's background, stone-dark (20,18,16) or stone-light (251,250,248) |
| Post-AI mode switch (General) | the switch at the right of the "Post-AI mode" label's row | Saved post_ai true, then false; on: at least 60 px of the theme's primary (235,230,223) in the switch's area; off: fewer than 5 primary px and at least 30 px of the knob's muted_foreground (160,151,141) |
| Font size field (Appearance) | the field showing 14 on the "Font size" row | Ctrl+A, "13", Enter: saved font_size becomes 13 (one step down, as the old minus did) |
| Tools panel Off / On (Appearance) | the Off, then the On word on the "Tools panel" row | Saved tools_panel "off", then "on" or absent, never rails; pixel (128,700) changes sidebar (15,13,11) → background (20,18,16) → sidebar |
| Agents page Remove button | found by its word "Remove" | At least 20 px of the button's border, input over background (49,45,41), within 16 × 12 px of the word |
| Keys page chip | found by its word "Ctrl" | At least 20 px of the chip's muted fill (31,28,25) within 6 px of the word |
| Title bar tools toggle, twice | (23,19) | Pixel (128,700) becomes the theme's background, then its sidebar again |

The synthetic initial theme copies Stone but changes only its background to #010203. This makes clicking
Stone prove a real colour change. It is fixture input, not a new expectation taken from renderer output.
System checks the saved choice, not a host-dependent desktop preference.

Every expected colour is computed by the script from `assets/themes/*.json` (the click fixture until Stone is
clicked, then stone-dark or stone-light by the mode Follow Off saved): a token's #rrggbb as bytes, and a #rrggbbaa
token composed over the theme's background and rounded (Stone input #fff0e11f over #141210 = (49,45,41)). None is
read from the build. The numbers in the table are Stone's, for the reader.

## Settings with every setting (C1, 2026-11-14)

C1 put every config key in Settings: the nav has seven groups, General, Appearance, Terminal, Editor, Agents,
History, Keys, and rows moved. These checks no longer click fixed points of the old layout:

- Settings opens with Ctrl+, (was the gear at (1217,19)); the frame, nav and page box come from the C1 gate's
  `open_settings`. Nav names are found by their words (was (330,154+34×index)); all seven pages are opened.
- Post-AI: the switch moved to the right of its row (the old point (955,197) read (20,18,16) where (235,230,223)
  was expected). The label row is found by OCR and the switch is the rightmost mark on it (C1 `label_at`,
  `row_band`, `rightmost_mark`).
- Size minus at (888,288) is gone: Font size is a field on Appearance. Typing 13 into it keeps the old outcome,
  14 → 13.
- The Tools panel row at (500,450) on Terminal is now an On | Off pair on Appearance; its Off and On words are
  clicked, with the same saved values and pixel (128,700) results.
- Agents (980,250) and Keys (975,172) pixels: the same colours are now looked for around the "Remove" and "Ctrl"
  words, wherever those rows are drawn.

Words are read more than one way (the C1 gate's `readings`: tesseract at scale/psm 3/11, 4/6, 4/11 and a
contrast-stretched reading); a word is found when one reading has it.

Before C1, Chrome measured panel-relative nav rectangles (9,58+34×index,183,32), mode centres about (594,99),
(650,99), (713,99), first theme card (229,185.84375,121.5,88), and minus centre (658,207.765625).
The section row spacing and colours come from the sheet; the Agents page uses its standard row controls.

`CLANKY_GATE_FAULT=click` drops the Stone row's pointer event at the test boundary. The unchanged
Stone pixel expectation must fail. No app/layout code or expected values are changed by the fault.
Results and real-window PNGs go under `$CARGO_TARGET_DIR/look/settings-clicks/`. No full gate runs here.
