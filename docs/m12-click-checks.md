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
| Each of General, Appearance, Terminal, Agents, Keys | (330,154+34×index) | Selected nav fill at (410,154+34×index) is (34,31,28); other rows (20,18,16) |
| Theme picker row: Stone in the Dark (left) column | found by its word in the left half of the Appearance body, scrolling if needed | theme_dark becomes stone-dark; body pixel (997,600) changes from (1,2,3) to (20,18,16) |
| Follow the system On / Off | its On / Off words on the Follow row | Config theme becomes system, then dark or light (the system's mode); body pixel (997,600) is that theme's background, (20,18,16) or (251,250,248) |
| Size minus | (888,288) | Starting at 14, saved font_size becomes 13 |
| Tools panel label (Terminal page) | (500,450) | Saved tools_panel "off", then "on" or absent, never rails; pixel (128,700) changes 15/13/11 → 20/18/16 → 15/13/11 |
| Post-AI label | (500,175) | Saved post_ai true, then false; switch pixel (955,197) is 235/230/223 on, 26/24/22 off |
| Agents page button border | Pixel (980,250) | Input border over background: (54,50,47) |
| Keys page chip | Pixel (975,172) | Muted fill (38,34,31) |
| Title bar tools toggle, twice | (23,19) | Pixel (128,700) becomes pane background, then the tools panel again |

The synthetic initial theme copies Stone but changes only its background to #010203. This makes clicking
Stone prove a real colour change. It is fixture input, not a new expectation taken from renderer output.
System checks the saved choice, not a host-dependent desktop preference. Input border RGB is #fff0e1 at
alpha 31 over #1a1816, rounded to bytes.

Chrome measured panel-relative nav rectangles (9,58+34×index,183,32), mode centres about (594,99),
(650,99), (713,99), first theme card (229,185.84375,121.5,88), and minus centre (658,207.765625).
The section row spacing and colours come from the sheet; the Agents page uses its standard row controls.

`CLANKY_GATE_FAULT=click` drops the Stone row's pointer event at the test boundary. The unchanged
Stone pixel expectation must fail. No app/layout code or expected values are changed by the fault.
Results and real-window PNGs go under `$CARGO_TARGET_DIR/look/settings-clicks/`. No full gate runs here.
