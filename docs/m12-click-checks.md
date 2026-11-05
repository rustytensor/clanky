# Remaining M12 Settings click checks

Command: `CARGO_TARGET_DIR=<target> scripts/check-settings-clicks.sh`.
The script takes the shared build lock. Set `CLANKY_GATE_BINARIES_READY=1` for matching prebuilt binaries;
`CLANKY_GATE_BIN_DIR` can select captured binaries. Call outside an outer lock.
Uses private Xvfb, xdotool, Pillow capture and a private server/home/config. No desktop input is sent.

Scope split with clanky-pkg, checked against its `364ab69`: its script owns access-row permission,
Settings open/Escape/close, Light mode, stepper plus, animations, disabled controls and block-hover work.
This script uses opening/closing as setup, without repeating those acceptance checks.

## Expectations fixed before the run

Sources: approved `mockups/settings.html`, independent Chrome bounds with bundled Geist fonts,
and check 8 of `docs/design-m12-calm.md`. Window 1240×760, scale 1, opacity 1.
Panel 780×600, centred at (230,80). RGB tolerance is 3 per channel.

| Remaining control | Click point | Expected result |
|---|---|---|
| Each of General, Appearance, Terminal, Agents, Keys | (330,154+34×index) | Selected nav fill at (410,154+34×index) is (34,31,28); other rows (20,18,16) |
| First theme card: Stone | (520,300) | theme_dark becomes stone-dark; body pixel (997,600) changes from (1,2,3) to (26,24,22) |
| System / Dark mode | (943,179) / (880,179) | Config theme becomes system / dark; Dark body pixel is (26,24,22) |
| Size minus | (888,288) | Starting at 14, saved font_size becomes 13 |
| Side columns label | (500,450) | Saved rails false, then true; sidebar pixel (128,700) changes 20/18/16 → 26/24/22 → 20/18/16 |
| Post-AI label | (500,175) | Saved post_ai true, then false; switch pixel (955,197) is 235/230/223 on, 26/24/22 off |
| Agents page button border | Pixel (980,250) | Input border over background: (54,50,47) |
| Keys page chip | Pixel (975,172) | Muted fill (38,34,31) |
| Sidebar toggle, shown / hidden | (288,36) / (40,36) | Sidebar pixel (128,700) becomes pane background, then sidebar again |

The synthetic initial theme copies Stone but changes only its background to #010203. This makes clicking
Stone prove a real colour change. It is fixture input, not a new expectation taken from renderer output.
System checks the saved choice, not a host-dependent desktop preference. Input border RGB is #fff0e1 at
alpha 31 over #1a1816, rounded to bytes.

Chrome measured panel-relative nav rectangles (9,58+34×index,183,32), mode centres about (594,99),
(650,99), (713,99), first theme card (229,185.84375,121.5,88), and minus centre (658,207.765625).
The section row spacing and colours come from the sheet; the Agents page uses its standard row controls.

`CLANKY_GATE_FAULT=click` drops the theme-card pointer event at the test boundary. The unchanged
Stone pixel expectation must fail. No app/layout code or expected values are changed by the fault.
Results and real-window PNGs go under `$CARGO_TARGET_DIR/look/settings-clicks/`. No full gate runs here.
