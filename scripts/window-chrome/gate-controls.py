#!/usr/bin/env python3
"""Controls gate: every control of clanky-gpui's main window and pane menu is drawn and works without a flaw, as the design
rulebook's section "Controls: drawn and working without a flaw" and its 2026-11-17 amendments ask.

Usage: gate-controls.py [--screens NAME,NAME...] [--scales 1,1.5] [--theme graphite-dark] [--app PATH]
                        [--simulate] [--fault NAME] [--keep] [--floating] [--test-popover]
Prints "FAIL: <check> <screen> @<scale> <control>: expected ..., observed ..." for each failure and one
"ok: <check> <screen> @<scale> ... (n checked)" line per passed check and screen, then "PASS: controls" when nothing
failed. Exit status 0 only on PASS.

Retargeted from the calm window's gate (design-tokens branch). What the gate reads, and how:
  - the control record the app writes (RECORD.md; env CLANKY_CONTROL_RECORD): which controls exist, where, of what kind
    and in what state, in device pixels;
  - screenshots of a private Xvfb, pointer and keys through xdotool (chrome_harness.RealWindow, which keeps G1's
    isolation rules);
  - every expected value from expected.py (rulebook-tokens.json and the Inter font files), at the scale being run; none
    from the program under test.
--simulate runs the same gate on simulated_window.SimWindow (no app, no display), with --fault NAME planting one
of the faults below in it; the gate must then FAIL the check named for the fault.

Checks, for each screen and scale (the check's name is the first word after FAIL):
  record        every control has every field of RECORD.md and a kind of the rulebook.
  whole-pixels  every box in the record is whole device pixels, and under the pointer the fill's four edges are sharp.
  height        the record's box height per kind, and the pressed fill's height on screen, +-1 device px.
  radius        the pressed fill's top-left corner: the area it leaves out of its corner square is r^2 (1 - pi/4).
  border-width  under keyboard focus, at the middle of the left edge: the border is one device pixel in ring, the pixel
                inside it is the focus fill, outside it the 2 px ring (round(2 x scale) device px) at 25 %.
  text-centring the label's baseline, fitted to the label's own ink rows against the Inter file at 100 x size, sits
                where the cap height's middle falls on the box's middle, +-0.5 device px.
  icon-centring the icon's ink is centred in the box +-0.5 device px, and its box is 16 or 14 px x the scale.
  rest-colour, hover-colour, pressed-colour, focus-colour  the fill per state (a field and a switch keep their look).
  disabled      a disabled control takes no hover fill and its label's ink is at most halfway to the foreground.
  hit-box       the record's hit box is its drawn box and at least 24 x 24 logical px; the pointer 1 device px inside
                each edge hovers the control and 1 device px outside it does not.
  release       pressing a control fires nothing; moving out before the release fires nothing and leaves the screen
                open; releasing inside fires the control exactly once, by "release".
  tab-order     from the pane (no control focused), Ctrl+F6 reaches the first enabled control and Tab visits every
                other once in reading order; Shift+Tab walks back (the rulebook's focus rule, 2026-11-17).
  arrows        in a group, from its first control (reached as tab-order does), Down reaches the second and Up the
                first again; in a horizontal group the same with Right and Left. Arrows in a pane are the program's.
  activate-keys Enter, and separately Space, fire the focused control exactly once, by "key".
  switch-space  Space flips a focused switch's checked value.
  escape        Esc closes a popover; the focus goes back to its opener (typed text reaches the pane it opened on).
  popover       a popover lies fully inside the window and does not cover what opened it, also by the corner.
Firing a control runs its action, so release and activate-keys fire only the screen's harmless control (SCREENS below).

Planted faults (--simulate --fault NAME; each must fail the check named):
  hit-larger  every hit box 4 px larger than the drawn box      -> hit-box
  press-fires a control fires on press, not on release           -> release
  tab-skip    the third control in reading order skips Tab       -> tab-order
  border-2px  the focus border drawn two device pixels wide      -> border-width
"""

import math
import os
import re
import shutil
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


FAULT = arg("--fault")
FAULTS = {
    "hit-larger": "hit-box",
    "press-fires": "release",
    "tab-skip": "tab-order",
    "border-2px": "border-width",
    # G3's faults, planted in the floating simulation (--floating).
    "popover-over-opener": "popover",
    "esc-loses-focus": "focus-return",
    "idle-draw": "idle-frames",
    "find-count-off": "find-count",
    "find-mark-accent": "find-highlight",
    "shift-enter-next": "find-previous",
    "tab-traps-completion": "tab-completes",
    "click-takes-focus": "click-focus",
    "active-tab-grows": "tab-stable",
    "selected-segment-hover": "hover-colour",
    "palette-no-run": "filter",  # the palette drops its offer to run the typed text (real window)
    # G4's faults, planted in the Settings simulation (--settings). card-padding is the design gate's.
    "setting-not-written": "config-roundtrip",
    "switch-hit-larger": "hit-box",
    "page-control-out-of-tab-order": "tab-order",
}
SIMULATE = "--simulate" in ARGS
if (
    SIMULATE
):  # a simulated window answers at once: pauses move a clock, they do not wait
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import virtual_clock

    virtual_clock.install()
SETTINGS = "--settings" in ARGS  # the simulation includes G4's Settings window
FLOATING = "--floating" in ARGS  # the simulation includes G3's floating screens
SCALES = [float(v) for v in arg("--scales", "1,1.5").split(",")]
THEME = arg("--theme", "graphite-dark")
KEEP = "--keep" in ARGS
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
FONTS = REPO / "assets/fonts"
APP = Path(
    arg("--app", os.environ.get("CLANKY_GPUI_BIN", REPO / "target/release/clanky-gpui"))
)
WORK = Path(
    os.environ.get("CLANKY_CONTROLS_WORK", "/srv/devops/build/clanky-controls-gate-run")
)
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "floating"))
sys.path.insert(0, str(HERE.parent / "settings"))
import expected as expected_values  # noqa: E402
from chrome_harness import Fail, RealWindow, until  # noqa: E402
from simulated_window import SimWindow  # noqa: E402
import chrome_pixels  # noqa: E402
from chrome_pixels import blend, grey, interior_mode  # noqa: E402
from chrome_pixels import corner_area_radius as corner_radius  # noqa: E402

# Window size, and the config every run starts from: no restored layout, no history, no animation, the dark theme.
LOGICAL_W, LOGICAL_H = 1200, 800
CONFIG = dict(
    restore_layout=False,
    # One pane, as the screens are defined: a second startup pane (new_window_panes, default 2, checked by the c5
    # gate) would take the keys from the first pane the checks type into.
    new_window_panes=1,
    startup_animation=False,
    history=False,
    animations=False,
    cursor_blink="off",
    theme="dark",
)

# ---------------------------------------------------------------- expected values, from expected.py at the run's scale

GHOST = {
    "menu-item",
    "list-row",
    "palette-row",
    "tab",
    "icon-button",
    "icon-button-inline",
    "button-bar",
    "segment",
}
OUTLINED = {"button-small", "button", "button-large", "chip"}
ROWS = {"menu-item", "list-row", "palette-row"}
KINDS = {}
INLINE_FILL_ALPHA = {}  # state -> share of the foreground over the container, for the tab close button
BACKGROUND = CARD = ACCENT = PRESSED = RING = FOREGROUND = PRIMARY = PRIMARY_PRESSED = 0
RING_ALPHA = DISABLED_ALPHA = TAB_ACTIVE_FILL = TAB_RING_ALPHA = INPUT_OVER = (
    INPUT_ALPHA
) = 0.0
COLOUR_TOL = EDGE_TOL = CENTRE_TOL = MIN_HIT = 0
INTER_UPM = INTER_CAP = 0
EXPECTED = {}


def grey_of(expected, role):
    """The grey level of a theme role (the Graphite colours are neutral greys)."""
    r, g, b = (int(expected["colours"][role]["hex"][i : i + 2], 16) for i in (1, 3, 5))
    return round((r + g + b) / 3)


def use_expected(scale, theme):
    """Loads expected.py's values for this scale and theme into the module's names, before each window."""
    global \
        KINDS, \
        INLINE_FILL_ALPHA, \
        BACKGROUND, \
        CARD, \
        ACCENT, \
        PRESSED, \
        RING, \
        FOREGROUND, \
        PRIMARY, \
        PRIMARY_PRESSED, \
        RING_ALPHA, \
        DISABLED_ALPHA
    global \
        TAB_RING_ALPHA, \
        INPUT_OVER, \
        INPUT_ALPHA, \
        TAB_ACTIVE_FILL, \
        COLOUR_TOL, \
        EDGE_TOL, \
        CENTRE_TOL, \
        MIN_HIT, \
        INTER_UPM, \
        INTER_CAP, \
        EXPECTED
    EXPECTED = expected_values.build(scale, theme)
    tokens = expected_values.load_tokens()
    KINDS = {}
    for kind, spec in EXPECTED["controls"].items():
        radius = None if kind in ("switch", "slider") else spec["radius"]["logical"]
        KINDS[kind] = (
            spec["height"]["logical"],
            radius,
            spec.get("text_px"),
            spec.get("text_weight"),
            spec["icon"]["logical"],
        )
    BACKGROUND, CARD, ACCENT, PRESSED, RING = (
        grey_of(EXPECTED, r)
        for r in ("background", "card", "accent", "pressed", "ring")
    )
    FOREGROUND, PRIMARY, PRIMARY_PRESSED = (
        grey_of(EXPECTED, r) for r in ("foreground", "primary", "primary_pressed")
    )
    INLINE_FILL_ALPHA = {
        state: EXPECTED["controls"]["icon-button-inline"]["fill_alpha"][state]
        for state in ("hover", "pressed")
    }
    RING_ALPHA = EXPECTED["states"]["focus"]["ring_alpha"]
    DISABLED_ALPHA = EXPECTED["states"]["disabled"]["opacity"]
    TAB_ACTIVE_FILL = tokens["active_tab"]["fill_foreground_alpha"]["value"]
    TAB_RING_ALPHA = tokens["active_tab"]["inner_ring_foreground_alpha"]["value"]
    INPUT_OVER = round(
        sum(int(EXPECTED["colours"]["input"]["over"][i : i + 2], 16) for i in (1, 3, 5))
        / 3
    )
    INPUT_ALPHA = EXPECTED["colours"]["input"]["alpha"]
    COLOUR_TOL = EXPECTED["colour_tolerance_levels"]["value"]
    EDGE_TOL = EXPECTED["chrome"]["tab_height"]["tolerance_px"]
    CENTRE_TOL = EXPECTED["controls"]["button"]["centring_tolerance_px"]
    MIN_HIT = tokens["geometry_rules"]["hit_box_min_px"]["value"]
    from fontTools.ttLib import TTFont

    font = TTFont(str(FONTS / "Inter-Regular.ttf"))
    glyph = font["glyf"][font.getBestCmap()[ord("H")]]
    glyph.recalcBounds(font["glyf"])
    INTER_UPM, INTER_CAP = font["head"].unitsPerEm, glyph.yMax


# The screens, how to open and close each, the colour of what holds its controls (None: the pixel just outside each
# control, as drawn), and the one control the gate may fire (its label), chosen because its action changes nothing
# the rest of the run depends on. A screen that names none fails activate-keys and release until it names one.


def open_tool_view(label):
    """Opens the main window with the tools panel showing one tool, chosen through its segment as a person would."""

    def open_it(win):
        win.away()
        segment = next(
            (
                c
                for c in (win.screen("main-window") or {}).get("controls", [])
                if c["kind"] == "segment" and c["label"] == label
            ),
            None,
        )
        if segment is None:
            raise Fail(f"no switcher segment {label} in the record")
        if not segment["checked"]:
            win.move(*centre(segment["box"]))
            win.x("mousedown", "1")
            win.x("mouseup", "1")
            until(
                lambda: any(
                    c["label"] == label and c["checked"]
                    for c in (win.screen("main-window") or {}).get("controls", [])
                ),
                f"the {label} tool is shown",
                3,
            )
        win.away()

    return open_it


def open_context_menu(win, at=None):
    x, y = at or win.menu_point()
    win.x("mousemove", "--window", win.wid, str(int(x)), str(int(y)))
    time.sleep(0.2)
    win.x("click", "3")


def count_label(query, total):
    """The find bar's count as today's window words it (calm `scene.rs` `find_bar`)."""
    if not query:
        return ""
    if total == 0:
        return "No matches"
    return "1 match" if total == 1 else f"{total} matches"


def close_with_escape(win):
    win.key("Escape")


# G3's screens. The context menu opens as the window opens it, on a right-click in a pane. TODO(G3): every other open
# key and harmless control below is a guess until G3's key bindings exist, except the palette's, which are the real
# ones (Ctrl+Shift+P opens it; New tab changes nothing the run depends on).

def ranked_matches(labels, query):
    """The palette's matching rule, written apart from the program (clanky_state::command_palette's comment): case
    folded and trimmed; labels starting with the query first, then those with a word starting with it, then those
    holding it anywhere; scattered letters do not match; ties keep the list's order."""
    query = query.strip().lower()

    def rank(label):
        label = label.lower()
        if label.startswith(query):
            return 0
        if any(word.startswith(query) for word in re.split(r"[^0-9a-z]+", label)):
            return 1
        return 2 if query in label else None

    found = [(rank(label), place, label) for place, label in enumerate(labels)]
    return [label for r, _, label in sorted(f for f in found if f[0] is not None)]


def open_by_keys(*keys):
    def run(win):
        win.key(*keys)

    return run


def open_completion(win):
    win.type("ls scripts/gate-m")
    win.key("Tab")


FLOATING_SCREENS = {
    # The pane's menu opens on a right-click at the harness's menu point, inside the first pane.
    "context-menu": dict(
        open=open_context_menu,
        close=close_with_escape,
        container="card",
        harmless="Select all",
        flip=True,
    ),
    "palette": dict(
        open=open_by_keys("ctrl+shift+p"),
        close=close_with_escape,
        container="card",
        harmless="New tab",
        adjacent=False,  # a modal list centred at the top: no opener beside it
        filter="ranked",  # best matches first (clanky_state::command_palette)
        run_row=True,  # and last, the offer to run the typed text in the pane
        idle=True,
    ),
    "agent-picker": dict(
        open=open_by_keys("ctrl+shift+g"),
        close=close_with_escape,
        container="card",
        harmless="Fake Tool",
        adjacent=False,
        filter=True,
    ),
    # Tab after `ls scripts/gate-m` is the real key (today's input, kept by clanky-gpui); the files are the harness's.
    "completion-list": dict(
        open=open_completion,
        close=close_with_escape,
        container="card",
        harmless="scripts/gate-menu.sh",
        idle=True,
        tab_completes=True,  # Tab puts the highlighted row in (q2-input C1a) instead of walking the rows
    ),
    # The pane's find bar opens on Ctrl+Shift+F, as today's window does (Ctrl+F stays the shell's).
    "find-bar": dict(
        open=open_by_keys("ctrl+shift+f"),
        close=close_with_escape,
        container="card",
        harmless="Next",
        adjacent=False,  # over the pane's top right corner, not beside an opener
        find=True,
        stays_open=True,  # firing Next or Previous leaves the bar open
    ),
}
ROWS_FILE = HERE.parent / "settings/rows.json"


def open_settings_page(label):
    """Opens Settings with Ctrl+, and shows one page through its switcher segment, as a person would.
    TODO(G4): the shortcut and the switcher's labels are Today's Clanky's until G4's window has its own."""

    def open_it(win):
        win.away()
        win.key("ctrl+comma")
        until(
            lambda: (win.screen("settings") or {}).get("controls"), "Settings opens", 5
        )
        segment = next(
            (
                c
                for c in win.screen("settings")["controls"]
                if c["kind"] == "segment"
                and c["group"] == "settings-pages"
                and c["label"] == label
            ),
            None,
        )
        if segment is None:
            raise Fail(f"no Settings page {label} in the record")
        if not segment["checked"]:
            win.move(*centre(segment["box"]))
            win.x("mousedown", "1")
            win.x("mouseup", "1")
            until(
                lambda: (win.screen("settings") or {}).get("page") == label,
                f"the {label} page is shown",
                3,
            )
        win.away()

    return open_it


SETTINGS_PAGES = (
    "General",
    "Appearance",
    "Terminal",
    "Editor",
    "Agents",
    "History",
    "Keys",
)
SETTINGS_SCREENS = {
    f"settings-{page.lower()}": dict(
        record="settings",
        open=open_settings_page(page),
        close=close_with_escape,
        container=None,
        harmless=page,
        scroll=True,
        roundtrip=page,
    )
    for page in SETTINGS_PAGES
}
POPOVER_MAX_GAP_LOGICAL_PX = (
    16  # "next to what opened it" (G3 definition): at most this far from the opener
)

# The test popover of a clanky-gpui built with the test-popover feature (--test-popover): what every floating screen
# shares, checked before the screens exist. Ctrl+Shift+F11 opens it beside the focused control (else the tools panel
# button), Ctrl+Shift+F12 from a point near the bottom-right corner, where it must go up and left.
TEST_POPOVER_SCREENS = {
    "test-popover": dict(
        open=open_by_keys("ctrl+shift+F11"),
        close=close_with_escape,
        container="card",
        harmless="First",
        idle=True,
    ),
    "test-popover-corner": dict(
        open=open_by_keys("ctrl+shift+F12"),
        close=close_with_escape,
        container="card",
        harmless="First",
    ),
}

SCREENS = {
    # The tools panel shows one tool at a time; the main window is checked with each of the three, whose segment is the
    # screen's harmless control (clicking the shown tool's segment changes nothing).
    "main-window": dict(
        open=open_tool_view("Files"),
        close=None,
        container=None,
        harmless="Files",
        click_focus=True,
    ),
    "main-window-recent": dict(
        record="main-window",
        open=open_tool_view("Recent"),
        close=None,
        container=None,
        harmless="Recent",
    ),
    "main-window-agents": dict(
        record="main-window",
        open=open_tool_view("Agents"),
        close=None,
        container=None,
        harmless="Agents",
    ),
}
if FLOATING:
    SCREENS.update(FLOATING_SCREENS)
if SETTINGS:
    SCREENS.update(SETTINGS_SCREENS)
if "--test-popover" in ARGS:
    SCREENS.update(TEST_POPOVER_SCREENS)

RECORD_FIELDS = {
    "id",
    "kind",
    "label",
    "box",
    "hit",
    "state",
    "enabled",
    "tab",
    "group",
    "axis",
    "checked",
    "primary",
    "label_box",
    "icon",
}

# ---------------------------------------------------------------- results

FAILED, PASSED = [], {}


def check(name, screen, scale, ok, what, expected, observed):
    key = (name, screen, scale)
    if ok:
        PASSED[key] = PASSED.get(key, 0) + 1
    else:
        line = f"FAIL: {name} {screen} @{scale:g} {what}: expected {expected}, observed {observed}"
        FAILED.append(line)
        print(line, flush=True)
    return ok


# ---------------------------------------------------------------- pixels


def fill_height(a, box, fill, radius, edge=None):
    return chrome_pixels.fill_height(a, box, fill, radius, edge, COLOUR_TOL)


def near(value, expected, tol=None):
    return abs(value - expected) <= (COLOUR_TOL if tol is None else tol)


def fill_sample(a, ctl, inset):
    """The control's fill colour: the commonest grey inside its border; for a switch, a point of the track clear of
    the knob (the knob sits at the end the value names)."""
    if ctl["kind"] != "switch":
        return interior_mode(a, ctl["box"], inset)
    x, y, w, h = ctl["box"]
    track_x = x + h // 2 if ctl.get("checked") else x + w - h // 2
    return grey(a, track_x, y + h // 2)


def hair(scale):
    """A border is one device pixel at every scale (rulebook, Amendments)."""
    return EXPECTED["chrome"]["hairline_device_px"]["device"]


def ring_width(scale):
    return EXPECTED["chrome"]["focus_ring_width"]["device_rounded"]


def radius_px(kind, box, scale):
    r = KINDS[kind][1]
    return box[3] / 2 if r is None else r * scale


def side_coverage(box, radius, px, py):
    """How much of device pixel (px, py) a rounded box with whole-pixel edges covers, for a pixel on its left or
    right edge: 1 on the straight part; on a round end, the antialiased coverage at the pixel's centre, half a pixel
    either side of the curve (clamp(0.5 - signed distance)). A pill's middle row lies half a pixel from its circles'
    centres when its height is even, so even a whole-pixel edge there is not fully covered."""
    x, y, w, h = box
    cx, cy = px + 0.5, py + 0.5
    top, bottom = y + radius, y + h - radius
    if top <= cy <= bottom or radius <= 0:
        return 1.0
    centre_y = top if cy < top else bottom
    centre_x = x + radius if cx < x + w / 2 else x + w - radius
    distance = math.hypot(cx - centre_x, cy - centre_y) - radius
    return min(1.0, max(0.0, 0.5 - distance))


_REFERENCE = {}


def reference_rows(label, px, weight):
    """The label rendered from the Inter file at 100 x px: ink per fine row, the baseline's fine row and fine rows
    per pixel."""
    key = (label, px, weight)
    if key not in _REFERENCE:
        size = int(round(px * 100))
        name = {400: "Regular", 500: "Medium", 600: "SemiBold"}[weight]
        font = ImageFont.truetype(str(FONTS / f"Inter-{name}.ttf"), size)
        left, top, right, bottom = font.getbbox(label, anchor="ls")
        im = Image.new("L", (right - left + 20, bottom - top + 20))
        base = 10 - top
        ImageDraw.Draw(im).text(
            (10 - left, base), label, font=font, fill=255, anchor="ls"
        )
        rows = np.asarray(im).astype(float).sum(axis=1)
        _REFERENCE[key] = (rows, base, size / px)
    return _REFERENCE[key]


def baseline_offset(a, ctl, scale, background):
    """Measured baseline minus the baseline of a label whose cap height is centred in the box, in device px."""
    kind = ctl["kind"]
    px, weight = KINDS[kind][2] * scale, KINDS[kind][3]
    if kind == "tab" and ctl.get("checked"):
        weight = 500
    x, y, w, h = ctl["box"]
    lx, _, lw, _ = ctl["label_box"]
    y0, y1 = int(y), int(y + h)
    x0, x1 = (
        max(int(math.floor(lx)) - 1, int(x)),
        min(int(math.ceil(lx + lw)) + 1, int(x + w)),
    )
    ink = np.abs(a[y0:y1, x0:x1].mean(axis=2) - background)
    ink[ink < 2] = 0
    measured = ink.sum(axis=1)
    if measured.sum() <= 0:
        return None
    measured = measured / measured.sum()
    rows, base, fine = reference_rows(ctl["label"], px, weight)
    cum = np.concatenate([[0.0], np.cumsum(rows)])

    def predicted(b):
        edges = (np.arange(y0, y1 + 1) - b) * fine + base
        p = np.diff(np.interp(edges, np.arange(len(cum)), cum))
        return p / p.sum() if p.sum() > 0 else p

    expected = y + h / 2 + INTER_CAP / INTER_UPM * px / 2
    candidates = np.arange(expected - 3, expected + 3, 0.01)
    errors = [((predicted(b) - measured) ** 2).sum() for b in candidates]
    return float(candidates[int(np.argmin(errors))] - expected)


def icon_centre_offset(a, ctl, background):
    """(across, down): the icon ink's middle minus the box's middle, in device px, from the ink's extent with the
    partly covered edge rows and columns counted by their coverage."""
    ix, iy, iw, ih = ctl["icon"]
    x0, y0, x1, y1 = (
        int(ix) - 1,
        int(iy) - 1,
        int(math.ceil(ix + iw)) + 1,
        int(math.ceil(iy + ih)) + 1,
    )
    ink = np.abs(a[y0:y1, x0:x1].mean(axis=2) - background)
    if ink.max() <= 0:
        return None
    ink = ink / ink.max()

    def extent(profile, origin):
        idx = np.where(profile > 0.02)[0]
        first, last = idx[0], idx[-1]
        return origin + first + 1 - profile[first], origin + last + profile[last]

    top, bottom = extent(ink.max(axis=1), y0)
    left, right = extent(ink.max(axis=0), x0)
    bx, by, bw, bh = ctl["box"]
    return (left + right) / 2 - (bx + bw / 2), (top + bottom) / 2 - (by + bh / 2)


# ---------------------------------------------------------------- the checks


def contains(outer, inner):
    """True when box `inner` lies wholly within box `outer`."""
    ox, oy, ow, oh = outer
    ix, iy, iw, ih = inner
    return ox <= ix and oy <= iy and ix + iw <= ox + ow and iy + ih <= oy + oh


def inside(box, x, y):
    return box[0] <= x < box[0] + box[2] and box[1] <= y < box[1] + box[3]


def rect_distance(a, b):
    """The distance between two boxes [x, y, w, h]: 0 when they touch or overlap."""
    dx = max(a[0] - (b[0] + b[2]), b[0] - (a[0] + a[2]), 0)
    dy = max(a[1] - (b[1] + b[3]), b[1] - (a[1] + a[3]), 0)
    return math.hypot(dx, dy)


def covers(box, opener):
    """Whether a popover's box covers its opener: overlaps a control's box, or holds the point of a right-click (an
    opener of no size), counting the box's top and left edges as inside it."""
    x, y, w, h = opener
    if w == 0 and h == 0:
        return box[0] <= x < box[0] + box[2] and box[1] <= y < box[1] + box[3]
    return overlaps(box, opener)


def overlaps(a, b):
    return (
        a[0] < b[0] + b[2]
        and b[0] < a[0] + a[2]
        and a[1] < b[1] + b[3]
        and b[1] < a[1] + a[3]
    )


def values_match(got, expected, tolerance):
    """Equal values; numbers may differ by the tolerance (a stored float is not an exact decimal)."""
    numbers = (int, float)
    if (
        isinstance(got, numbers)
        and isinstance(expected, numbers)
        and not isinstance(got, bool)
        and not isinstance(expected, bool)
    ):
        return abs(got - expected) <= tolerance
    return got == expected


def centre(box):
    return int(box[0] + box[2] / 2), int(box[1] + box[3] / 2)


def reading_order(controls):
    return [
        c["id"]
        for c in sorted(
            controls, key=lambda c: (round(c["box"][1]), round(c["box"][0]))
        )
    ]


def name_of(ctl):
    return f'{ctl["kind"]} "{ctl["label"]}"'


def border_blend(ctl, fill):
    """The grey the outermost pixel of a control shows: its fill under its border (the border is drawn on the fill)."""
    kind = ctl["kind"]
    if kind in OUTLINED or kind in ("field", "switch", "slider"):
        over, alpha = INPUT_OVER, INPUT_ALPHA
    elif kind == "tab" and ctl.get("checked"):
        over, alpha = FOREGROUND, TAB_RING_ALPHA
    else:
        return fill
    return blend(fill, over, alpha)


def expected_fill(ctl, state, container):
    kind = ctl["kind"]
    if kind == "icon-button-inline":
        # No fill at rest; hover and press lay the foreground over whatever is under it (rulebook, Amendments).
        alpha = INLINE_FILL_ALPHA.get(state)
        return (
            container if alpha is None else round(blend(container, FOREGROUND, alpha))
        )
    if kind == "slider":
        return ACCENT  # a track of one look in every state, as the switch
    if kind == "switch":
        return (
            PRIMARY if ctl.get("checked") else ACCENT
        )  # one look in every state (rulebook, Amendments)
    if ctl.get("primary"):
        return {
            "rest": PRIMARY,
            "hover": PRIMARY_PRESSED,
            "pressed": PRIMARY_PRESSED,
            "focus": PRIMARY,
        }[state]
    rest = container
    if kind == "tab" and ctl.get("checked"):
        rest = round(blend(container, FOREGROUND, TAB_ACTIVE_FILL))
    elif kind == "segment" and ctl.get("checked"):
        # The selected segment keeps its own fill under the pointer and in focus; only a press changes it.
        return PRESSED if state == "pressed" and ctl["enabled"] else BACKGROUND
    elif kind in ROWS and ctl.get("checked"):
        rest = ACCENT  # the selected row of a list (rulebook token table: accent is "hover, selected row")
    if state == "rest" or not ctl["enabled"] or kind == "field":
        return rest
    if state == "hover":
        return ACCENT
    if state == "pressed":
        return PRESSED
    return ACCENT if kind in ROWS else rest


class Screen:
    def __init__(self, win, name, spec):
        self.win, self.name, self.spec, self.s = win, name, spec, win.scale
        self.record_name = spec.get("record", name)  # the screen's name in the record
        self.record = None

    def ok(self, check_name, ok, what, expected, observed):
        return check(check_name, self.name, self.s, ok, what, expected, observed)

    def current(self):
        return self.win.screen(self.record_name)

    def controls(self):
        """The controls in view: a page that scrolls records the others too, with `visible` false."""
        r = self.current()
        return [c for c in r["controls"] if c.get("visible", True)] if r else []

    def control(self, cid):
        r = self.current()
        return next((c for c in (r["controls"] if r else []) if c["id"] == cid), None)

    def is_open(self):
        return bool(self.controls())

    def open(self, **kw):
        if self.spec["close"] and self.is_open():
            return self.current()
        self.spec["open"](self.win, **kw)
        try:
            until(self.is_open, f"{self.name} opens and records its controls", 5)
        except Fail:
            return None
        time.sleep(0.3)
        self.record = self.in_view(self.current())
        self.all_at_open = list(self.current()["controls"])
        return self.record

    @staticmethod
    def in_view(record):
        """The record with only the controls in view (the same record when everything is)."""
        return dict(
            record, controls=[c for c in record["controls"] if c.get("visible", True)]
        )

    def close(self):
        if self.spec["close"] and self.is_open():
            self.spec["close"](self.win)
            try:
                until(lambda: not self.is_open(), f"{self.name} closes", 3)
            except Fail:
                pass

    def clear_focus(self):
        """No control has the focus: a click on empty chrome ground (which leaves the pane its keyboard), then Escape
        if a control still has it, which gives the keyboard back to the pane."""
        x, y = self.rest_point()
        self.win.move(x, y)
        self.win.x("mousedown", "1")
        self.win.x("mouseup", "1")
        time.sleep(0.3)
        if (self.current() or {}).get("focus") is not None:
            self.win.key("Escape")
            time.sleep(0.3)

    def reopen(self, **kw):
        self.close()
        self.clear_focus()
        return self.open(**kw)

    def container(self, a, ctl):
        if self.spec["container"] == "card":
            return CARD
        if ctl["kind"] == "segment":
            return self.tray_colour(a, ctl)
        x, y, w, h = ctl["box"]
        return grey(a, x - ring_width(self.s) - 2, y + h / 2)

    def tray_colour(self, a, ctl):
        """The switcher tray's colour, read off another unselected segment of the same switcher (a segment's own
        surroundings are covered by its focus ring), which shows the tray at rest. A window can hold switchers on
        different grounds (the tools panel's footer, a pane's command bar), so only its own group's tray counts."""
        other = next(
            (
                c
                for c in self.controls()
                if c["kind"] == "segment"
                and c.get("group") == ctl.get("group")
                and c["id"] != ctl["id"]
                and not c.get("checked")
                and c["enabled"]
            ),
            None,
        )
        if other is not None:
            return fill_sample(a, other, hair(self.s) + 1)
        # No other unselected segment (a two-segment switcher with its sibling selected): the tray just above that
        # sibling, which this segment's focus ring does not cover.
        sibling = next(
            (
                c
                for c in self.controls()
                if c["kind"] == "segment"
                and c.get("group") == ctl.get("group")
                and c["id"] != ctl["id"]
            ),
            ctl,
        )
        x, y, w, _ = sibling["box"]
        return grey(a, x + w / 2, y - 1)

    def rest_point(self):
        """A pointer position over no control and inside no popover: the popover's opener when there is one."""
        r = self.current()
        if r and r.get("popover"):
            ox, oy, ow, oh = r["popover"]["opener"]
            if not any(inside(c["hit"], ox, oy) for c in r["controls"]) and not inside(
                r["popover"]["box"], ox, oy
            ):
                return int(ox), int(oy)
        # Chrome ground, never a pane: a click there leaves the pane its keyboard, a click in a pane gives it.
        panes = (self.win.landmarks() or {}).get("pane") or []
        for fy in (0.5, 0.95, 0.2, 0.8, 0.985, 0.02):
            for fx in (0.95, 0.6, 0.3):
                x, y = int(fx * self.win.pw), int(fy * self.win.ph)
                if (
                    not any(inside(c["hit"], x, y) for c in self.controls())
                    and not any(inside(box, x, y) for box in panes)
                    and not (
                        r and r.get("popover") and inside(r["popover"]["box"], x, y)
                    )
                ):
                    return x, y
        return 2, 2

    def outside_point(self, ctl):
        """A point 2 device px outside the control and outside every control, inside the popover if any."""
        x, y, w, h = ctl["box"]
        cx, cy = centre(ctl["box"])
        pop = (self.current() or {}).get("popover")
        for px_, py_ in (
            (x - 2, cy),
            (x + w + 1, cy),
            (cx, y - 2),
            (cx, y + h + 1),
            (x - 2 - ring_width(self.s), cy),
        ):
            if any(
                inside(c["box"], px_, py_) or inside(c["hit"], px_, py_)
                for c in self.controls()
            ):
                continue
            if pop and not inside(pop["box"], px_, py_):
                continue
            return px_, py_
        return self.rest_point()

    def height_of(self, c):
        """A control's height in device px: its kind's, or for an icon button inside another control (a tab's close,
        a row's action) the rulebook's in-row height."""
        in_rows = EXPECTED["controls"][c["kind"]].get("height_in_rows")
        if in_rows and any(
            o is not c and contains(o["box"], c["box"]) for o in self.record["controls"]
        ):
            return in_rows["logical"] * self.s
        return KINDS[c["kind"]][0] * self.s

    def wait_state(self, cid, state, timeout=3.0):
        try:
            return until(
                lambda: (self.control(cid) or {}).get("state") == state, "", timeout
            )
        except Fail:
            return False

    # Each check below reads the record taken when the screen opened.
    def static_checks(self):
        r = self.record
        for c in r["controls"]:
            missing = sorted(RECORD_FIELDS - set(c))
            if not self.ok(
                "record",
                not missing and c.get("kind") in KINDS,
                name_of(c) if "kind" in c else "control",
                "every field of RECORD.md and a rulebook kind",
                f"missing {missing}, kind {c.get('kind')}",
            ):
                continue
            self.ok(
                "whole-pixels",
                all(abs(v - round(v)) < 0.01 for v in c["box"]),
                name_of(c) + " box",
                "whole device pixels",
                c["box"],
            )
            if c["kind"] not in KINDS:
                self.ok(
                    "height", False, name_of(c), "a kind of the rulebook", c["kind"]
                )
                continue
            want = self.height_of(c)
            self.ok(
                "height",
                abs(c["box"][3] - want) <= EDGE_TOL,
                name_of(c) + " recorded height",
                f"{want:g}",
                f"{c['box'][3]:g}",
            )
            same = all(abs(a - b) < 0.01 for a, b in zip(c["box"], c["hit"]))
            self.ok(
                "hit-box",
                same,
                name_of(c) + " recorded hit box",
                f"the drawn box {c['box']}",
                c["hit"],
            )
            small = min(c["hit"][2], c["hit"][3])
            self.ok(
                "hit-box",
                small >= MIN_HIT * self.s - 0.01,
                name_of(c) + " hit box size",
                f"at least {MIN_HIT * self.s:g} px each way",
                f"{c['hit'][2]:g} x {c['hit'][3]:g}",
            )
        pop = r.get("popover")
        if pop:
            b = pop["box"]
            on = (
                b[0] >= 0
                and b[1] >= 0
                and b[0] + b[2] <= self.win.pw
                and b[1] + b[3] <= self.win.ph
            )
            self.ok(
                "popover",
                on,
                "popover",
                f"inside the window 0,0 {self.win.pw}x{self.win.ph}",
                b,
            )
            self.ok(
                "popover",
                not covers(b, pop["opener"]),
                "popover",
                f"clear of its opener {pop['opener']}",
                b,
            )
            if self.spec.get("adjacent", True):
                gap = rect_distance(b, pop["opener"])
                limit = POPOVER_MAX_GAP_LOGICAL_PX * self.s
                self.ok(
                    "popover",
                    gap <= limit,
                    "popover",
                    f"next to its opener, at most {limit:g} px away",
                    f"{gap:.1f} px",
                )

    def rest_checks(self):
        self.win.away(self.rest_point())
        time.sleep(0.3)
        a = self.win.shot(f"{self.name}-rest")
        for c in self.controls():
            if c["kind"] not in KINDS:
                continue
            bg = self.container(a, c)
            want = expected_fill(c, "rest", bg)
            if want is not None:
                got = fill_sample(a, c, hair(self.s) + 1)
                self.ok(
                    "rest-colour",
                    got is not None and near(got, want),
                    name_of(c),
                    want,
                    got,
                )
            fill = want if want is not None else bg
            if c.get("label_box") and c["label"] and KINDS[c["kind"]][2]:
                off = baseline_offset(a, c, self.s, fill)
                self.ok(
                    "text-centring",
                    off is not None and abs(off) <= CENTRE_TOL,
                    name_of(c),
                    f"baseline within {CENTRE_TOL} px of the centred one",
                    "no ink" if off is None else f"{off:+.2f} px",
                )
            if c.get("icon"):
                size = KINDS[c["kind"]][4] * self.s
                self.ok(
                    "icon-centring",
                    abs(c["icon"][2] - size) <= 0.5 and abs(c["icon"][3] - size) <= 0.5,
                    name_of(c) + " icon box",
                    f"{size:g} px square",
                    c["icon"][2:],
                )
                off = icon_centre_offset(a, c, fill)
                across = c["kind"] == "icon-button"
                good = (
                    off is not None
                    and abs(off[1]) <= CENTRE_TOL
                    and (not across or abs(off[0]) <= CENTRE_TOL)
                )
                self.ok(
                    "icon-centring",
                    good,
                    name_of(c),
                    f"within {CENTRE_TOL} px of the middle",
                    "no ink" if off is None else f"{off[0]:+.2f}, {off[1]:+.2f} px",
                )
            if not c["enabled"] and c.get("label_box"):
                x, y, w, h = c["box"]
                lx, _, lw, _ = c["label_box"]
                region = a[int(y) : int(y + h), int(lx) : int(lx + lw)].mean(axis=2)
                peak = int(region.max()) if region.size else 0
                limit = round(blend(bg, FOREGROUND, DISABLED_ALPHA))
                self.ok(
                    "disabled",
                    peak <= limit + COLOUR_TOL and peak >= bg + 20,
                    name_of(c) + " label ink",
                    f"brightest at most {limit} (+{COLOUR_TOL}) and drawn",
                    peak,
                )

    def hover_checks(self):
        for c in self.record["controls"]:
            if c["kind"] not in KINDS or not self.is_open():
                self.open()
            cid = c["id"]
            cx, cy = centre(c["box"])
            self.win.move(cx, cy)
            if c["enabled"]:
                if not self.ok(
                    "hit-box",
                    self.wait_state(cid, "hover"),
                    name_of(c) + " under the pointer",
                    "hover",
                    (self.control(cid) or {}).get("state"),
                ):
                    continue
            else:
                time.sleep(0.3)
                hovered = [d["label"] for d in self.controls() if d["state"] == "hover"]
                self.ok(
                    "disabled",
                    not hovered
                    and (self.control(cid) or {}).get("state") == "disabled",
                    name_of(c) + " under the pointer",
                    "stays disabled, nothing hovered",
                    f"{(self.control(cid) or {}).get('state')}, hovered {hovered}",
                )
            time.sleep(0.15)
            a = self.win.shot(f"{self.name}-hover-{cid}")
            bg = self.container(a, c)
            want = expected_fill(c, "hover", bg)
            if want is not None:
                got = fill_sample(a, c, hair(self.s) + 1)
                self.ok(
                    "hover-colour" if c["enabled"] else "disabled",
                    got is not None and near(got, want),
                    name_of(c),
                    want,
                    got,
                )
            if not c["enabled"]:
                continue
            if want is not None and want != bg:
                x, y, w, h = (int(round(v)) for v in c["box"])
                rad = int(math.ceil(radius_px(c["kind"], c["box"], self.s)))
                ex = x + rad + 1
                pairs = (
                    ((x, cy), (x - 1, cy), "left"),
                    ((x + w - 1, cy), (x + w, cy), "right"),
                    ((ex, y), (ex, y - 1), "top"),
                    ((ex, y + h - 1), (ex, y + h), "bottom"),
                )
                exact = radius_px(c["kind"], c["box"], self.s)
                for (ix, iy), (ox, oy), side in pairs:
                    gi, go = grey(a, ix, iy), grey(a, ox, oy)
                    edge = border_blend(c, want)
                    if side in ("left", "right"):
                        edge = bg + side_coverage((x, y, w, h), exact, ix, iy) * (
                            edge - bg
                        )
                    outside = self.outside_fill(c, ox, oy, bg)
                    self.ok(
                        "whole-pixels",
                        near(gi, edge) and near(go, outside),
                        f"{name_of(c)} {side} edge",
                        f"{edge:.0f} then {outside}",
                        f"{gi} then {go}",
                    )
            # The pointer 1 px inside each edge hovers the control; 1 px outside does not.
            x, y, w, h = c["box"]
            ex = int(x + radius_px(c["kind"], c["box"], self.s) + 1)
            probes = (
                ((x, cy), (x - 1, cy)),
                ((x + w - 1, cy), (x + w, cy)),
                ((ex, y), (ex, y - 1)),
                ((ex, y + h - 1), (ex, y + h)),
            )
            for (ix, iy), (ox, oy) in probes:
                if not any(
                    d["id"] != cid and inside(d["box"], ox, oy) for d in self.controls()
                ):
                    self.win.move(ox, oy)
                    time.sleep(0.25)
                    st = (self.control(cid) or {}).get("state")
                    self.ok(
                        "hit-box",
                        st != "hover",
                        f"{name_of(c)} pointer at {ox},{oy}, 1 px outside",
                        "not hovered",
                        st,
                    )
                self.win.move(ix, iy)
                st = self.wait_state(cid, "hover", 1.5)
                self.ok(
                    "hit-box",
                    bool(st),
                    f"{name_of(c)} pointer at {ix},{iy}, 1 px inside",
                    "hover",
                    (self.control(cid) or {}).get("state"),
                )

    def outside_fill(self, ctl, x, y, bg):
        """What lies just outside a control's edge: its container, or the rest fill of a neighbouring row drawn in
        `accent` because it is the list's selected row (a list's rows touch, 28 px apart)."""
        for d in self.controls():
            if d["id"] != ctl["id"] and d.get("checked") and inside(d["box"], x, y):
                want = expected_fill(d, "rest", bg)
                return bg if want is None else want
        return bg

    def press_checks(self):
        """Press each enabled control, the harmless one first: the pressed look and size, nothing fired on press,
        then a move out and a release outside, which fires nothing."""
        enabled = [
            c for c in self.record["controls"] if c["enabled"] and c["kind"] in KINDS
        ]
        enabled.sort(key=lambda c: c["label"] != self.spec["harmless"])
        for c in enabled:
            if not self.is_open():
                self.open()
            cid = c["id"]
            self.win.move(*centre(c["box"]))
            self.wait_state(cid, "hover", 1.5)
            fired = len(self.win.activations(self.record_name))
            self.win.x("mousedown", "1")
            pressed = self.wait_state(cid, "pressed", 2)
            self.ok(
                "pressed-colour",
                pressed,
                name_of(c) + " state on press",
                "pressed",
                (self.control(cid) or {}).get("state")
                if self.is_open()
                else "screen closed",
            )
            time.sleep(0.15)
            a = self.win.shot(f"{self.name}-pressed-{cid}")
            on_press = len(self.win.activations(self.record_name)) - fired
            ok = self.ok(
                "release",
                on_press == 0,
                name_of(c) + " on press",
                "fires nothing",
                f"fired {on_press}",
            )
            if pressed:
                bg = self.container(a, c)
                want = expected_fill(c, "pressed", bg)
                if want is not None:
                    got = fill_sample(a, c, hair(self.s) + 1)
                    self.ok(
                        "pressed-colour",
                        got is not None and near(got, want),
                        name_of(c),
                        want,
                        got,
                    )
                if want is not None and want != bg:
                    rad = radius_px(c["kind"], c["box"], self.s)
                    hh = fill_height(a, c["box"], want, rad, border_blend(c, want))
                    self.ok(
                        "height",
                        abs(hh - self.height_of(c)) <= EDGE_TOL,
                        name_of(c) + " drawn",
                        f"{self.height_of(c):g} px",
                        f"{hh} px",
                    )
                    r = corner_radius(a, c["box"], want, bg, rad, border_blend(c, want))
                    self.ok(
                        "radius",
                        abs(r - rad) <= EDGE_TOL,
                        name_of(c),
                        f"{rad:g} px",
                        f"{r:.2f} px",
                    )
            self.win.move(*self.outside_point(c))
            time.sleep(0.2)
            self.win.x("mouseup", "1")
            time.sleep(0.4)
            after = len(self.win.activations(self.record_name)) - fired
            self.ok(
                "release",
                after == 0 and self.is_open(),
                name_of(c) + " released outside",
                "fires nothing, screen stays open",
                f"fired {after}, open {self.is_open()}",
            )
            if not ok or after:
                break  # a control that fires on its own would run every action of the screen

    def tab_checks(self):
        if not self.reopen():
            return
        self.win.away(self.rest_point())
        expected = reading_order([c for c in self.all_at_open if c["enabled"]])
        visited, last = [], None
        for i in range(len(expected)):
            # The pane has the keyboard: Ctrl+F6 enters the controls at the first, then Tab walks them.
            self.win.key("ctrl+F6" if i == 0 else "Tab")
            try:
                until(
                    lambda: (self.current() or {}).get("focus") not in (None, last),
                    "",
                    2,
                )
            except Fail:
                pass
            r = self.current() or {}
            last = r.get("focus")
            visited.append(last)
            c = self.control(last)
            if c is None:
                continue
            self.ok(
                "focus-colour",
                r.get("keyboard") is True and c["state"] == "focus",
                name_of(c) + " after Tab",
                "state focus, from the keyboard",
                f"{c['state']}, keyboard {r.get('keyboard')}",
            )
            if c["kind"] not in KINDS:
                continue
            time.sleep(0.15)
            a = self.win.shot(f"{self.name}-focus-{last}")
            self.focus_look(a, c)
        names = {c["id"]: c["label"] for c in self.all_at_open}
        self.ok(
            "tab-order",
            visited == expected,
            "Ctrl+F6 from the pane, then Tab",
            [names.get(i) for i in expected],
            [names.get(i) for i in visited],
        )
        back = []
        for i in range(len(expected) - 1):
            self.win.key("shift+Tab")
            prev = back[-1] if back else last
            try:
                until(lambda: (self.current() or {}).get("focus") != prev, "", 2)
            except Fail:
                pass
            back.append((self.current() or {}).get("focus"))
        want_back = list(reversed(expected))[1:]
        self.ok(
            "tab-order",
            back == want_back,
            "Shift+Tab back",
            [names.get(i) for i in want_back],
            [names.get(i) for i in back],
        )

    def focus_look(self, a, c):
        bg = self.container(a, c)
        want = expected_fill(c, "focus", bg)
        if want is not None:
            got = fill_sample(a, c, hair(self.s) + 2)
            self.ok(
                "focus-colour",
                got is not None and near(got, want),
                name_of(c),
                want,
                got,
            )
        x, y, w, h = c["box"]
        cy = int(y + h / 2)
        line = [
            grey(a, x + i, cy) for i in range(-ring_width(self.s) - 1, hair(self.s) + 1)
        ]
        ring_px = line[1 : 1 + ring_width(self.s)]
        border_px = line[1 + ring_width(self.s) : 1 + ring_width(self.s) + hair(self.s)]
        inner = line[-1]
        ring_want = blend(bg, RING, RING_ALPHA)
        inner_want = want if want is not None else bg
        if c["kind"] == "segment" and c.get("checked"):
            # A selected segment keeps its own fill, the theme's background, in every state (rulebook amendment
            # of 2026-11-17: hover's accent is for unselected segments only).
            inner_want = grey_of(EXPECTED, "background")
        good = (
            all(near(v, ring_want) for v in ring_px)
            and all(near(v, RING) for v in border_px)
            and near(inner, inner_want)
        )
        self.ok(
            "border-width",
            good,
            name_of(c) + f" left edge, row {cy}",
            f"ring {ring_want:.1f} x {ring_width(self.s)}, border {RING} x {hair(self.s)}, then {inner_want}",
            f"{ring_px} {border_px} {inner}",
        )

    def arrow_checks(self):
        groups = {}
        for c in self.record["controls"]:
            if c.get("group") and c["enabled"]:
                groups.setdefault((c["group"], c.get("axis")), []).append(c)
        for (group, axis), members in groups.items():
            if len(members) < 2 or not self.reopen():
                continue
            self.win.away(self.rest_point())
            order = reading_order(members)
            names = {c["id"]: c["label"] for c in members}
            if not self.focus_on(order[0]):
                self.ok(
                    "arrows",
                    False,
                    f"group {group}",
                    "its first member reached by Tab",
                    "never focused",
                )
                continue
            fwd, back = ("Down", "Up") if axis != "horizontal" else ("Right", "Left")
            seen = []
            for k in (fwd, back):
                before = (self.current() or {}).get("focus")
                self.win.key(k)
                try:
                    until(lambda: (self.current() or {}).get("focus") != before, "", 2)
                except Fail:
                    pass
                seen.append((self.current() or {}).get("focus"))
            want = [order[1], order[0]]
            self.ok(
                "arrows",
                seen == want,
                f"group {group}: {fwd}, {back} from its first member",
                [names.get(i) for i in want],
                [names.get(i) for i in seen],
            )

    def harmless(self):
        return next(
            (
                c
                for c in self.record["controls"]
                if c["label"] == self.spec["harmless"] and c["enabled"]
            ),
            None,
        )

    def focus_on(self, cid):
        for _ in range(len(self.controls()) + 1):
            if (self.current() or {}).get("focus") == cid:
                return True
            before = (self.current() or {}).get("focus")
            self.win.key("Tab" if before is not None else "ctrl+F6")
            try:
                until(lambda: (self.current() or {}).get("focus") != before, "", 2)
            except Fail:
                pass
        return (self.current() or {}).get("focus") == cid

    def fire_checks(self):
        h = self.harmless()
        if h is None:
            self.ok(
                "activate-keys",
                False,
                "harmless control",
                "one named for this screen and enabled",
                self.spec["harmless"],
            )
            self.ok(
                "release",
                False,
                "harmless control",
                "one named for this screen and enabled",
                self.spec["harmless"],
            )
            return
        pop = bool(self.record.get("popover"))
        for key in ("Return", "space"):
            if not self.reopen():
                return
            self.win.away(self.rest_point())
            if not self.focus_on(h["id"]):
                self.ok(
                    "activate-keys",
                    False,
                    f"{name_of(h)} {key}",
                    "reached by Tab",
                    "never focused",
                )
                continue
            fired = len(self.win.activations(self.record_name))
            self.win.key(key)
            time.sleep(0.6)
            new = self.win.activations(self.record_name)[fired:]
            good = (
                len(new) == 1
                and new[0]["id"] == h["id"]
                and new[0]["via"] == "key"
                and (not pop or self.spec.get("stays_open") or not self.is_open())
            )
            self.ok(
                "activate-keys",
                good,
                f"{name_of(h)} {key}",
                "fires once by key"
                + (", closes" if pop and not self.spec.get("stays_open") else ""),
                f"{[(n['label'], n['via']) for n in new]}, open {self.is_open()}",
            )
        if not self.reopen():
            return
        cx, cy = centre(h["box"])
        self.win.move(cx, cy)
        self.wait_state(h["id"], "hover", 1.5)
        fired = len(self.win.activations(self.record_name))
        self.win.x("mousedown", "1")
        time.sleep(0.4)
        on_press = len(self.win.activations(self.record_name)) - fired
        self.win.x("mouseup", "1")
        time.sleep(0.6)
        new = self.win.activations(self.record_name)[fired:]
        good = (
            on_press == 0
            and len(new) == 1
            and new[0]["id"] == h["id"]
            and new[0]["via"] == "release"
        )
        self.ok(
            "release",
            good,
            f"{name_of(h)} pressed and released inside",
            "nothing on press, once on release",
            f"{on_press} on press, then {[(n['label'], n['via']) for n in new]}",
        )

    def switch_checks(self):
        for c in [
            c for c in self.record["controls"] if c["kind"] == "switch" and c["enabled"]
        ]:
            if not self.reopen():
                return
            self.win.away(self.rest_point())
            if not self.focus_on(c["id"]):
                self.ok(
                    "switch-space", False, name_of(c), "reached by Tab", "never focused"
                )
                continue
            before = (self.control(c["id"]) or {}).get("checked")
            self.win.key("space")
            time.sleep(0.5)
            after = (self.control(c["id"]) or {}).get("checked")
            self.ok(
                "switch-space",
                before is not None and after == (not before),
                name_of(c),
                f"{before} flipped",
                after,
            )

    def escape_checks(self):
        if not self.record.get("popover") or not self.reopen():
            return
        pop = self.current()["popover"]
        self.win.key("Escape")
        try:
            until(lambda: not self.is_open(), "", 2)
        except Fail:
            pass
        self.ok(
            "escape",
            not self.is_open(),
            "Esc",
            "closes the popover",
            f"open {self.is_open()}",
        )
        if pop.get("opener_id") is not None:
            owners = [
                r
                for r in (self.win.screen(s) for s in SCREENS)
                if r and r.get("focus") == pop["opener_id"]
            ]
            self.ok(
                "escape",
                bool(owners) and owners[0].get("keyboard"),
                "focus after Esc",
                f"the opener {pop['opener_id']}",
                [(r["screen"], r.get("focus")) for r in owners],
            )
        elif pop.get("opener_pane"):
            mark = f"zq-focus-{self.s:g}".replace(".", "-")
            self.win.type(f"echo {mark}")
            self.win.key("Return")
            try:
                until(
                    lambda: self.win.pane_text(pop["opener_pane"]).count(mark) >= 2,
                    "",
                    4,
                )
                got = True
            except Fail:
                got = False
            self.ok(
                "escape",
                got,
                "typing after Esc",
                f"reaches pane {pop['opener_pane']}",
                got,
            )

    def click_focus_checks(self):
        """A pointer click on a bar control leaves the focus with the pane: typing afterwards reaches the pane."""
        if not self.spec.get("click_focus") or not self.reopen():
            return
        target = next(
            (
                c
                for c in self.controls()
                if c["enabled"]
                and c["kind"] not in ("field", "menu-item")
                and c["label"] == self.spec["harmless"]
            ),
            None,
        )
        if target is None:
            self.ok(
                "click-focus",
                False,
                "a bar control to click",
                self.spec["harmless"],
                "none",
            )
            return
        self.win.move(*centre(target["box"]))
        self.win.x("mousedown", "1")
        self.win.x("mouseup", "1")
        time.sleep(0.4)
        focus = (self.current() or {}).get("focus")
        self.ok(
            "click-focus",
            focus is None,
            f"a click on {name_of(target)}",
            "no control has the focus",
            focus,
        )
        mark = f"zq-click-{self.s:g}".replace(".", "-")
        self.win.type(f"echo {mark}")
        self.win.key("Return")
        pane = self.win.first_pane()
        try:
            until(lambda: self.win.pane_text(pane).count(mark) >= 2, "", 4)
            got = True
        except Fail:
            got = False
        self.ok(
            "click-focus", got, "typing after the click", f"reaches pane {pane}", got
        )

    def tab_stable_checks(self):
        """Switching tabs moves only `checked`: every tab keeps the box it had."""
        if not self.spec.get("click_focus") or not self.reopen():
            return

        def tab_boxes():
            tabs = [c for c in self.controls() if c["kind"] == "tab"]
            return {c["id"]: c["box"] for c in tabs}, {
                c["id"]: c["checked"] for c in tabs
            }

        before, active_before = tab_boxes()
        other = next(
            (c for c in self.controls() if c["kind"] == "tab" and not c["checked"]),
            None,
        )
        if other is None:
            self.ok("tab-stable", False, "an inactive tab to switch to", "one", "none")
            return
        self.win.move(*centre(other["box"]))
        self.win.x("mousedown", "1")
        self.win.x("mouseup", "1")
        time.sleep(0.5)
        self.win.away(self.rest_point())
        time.sleep(0.3)
        after, active_after = tab_boxes()
        self.ok(
            "tab-stable",
            active_after != active_before,
            "a click on another tab",
            "the active tab changes",
            active_after,
        )
        changed = {
            i: (before[i], after.get(i)) for i in before if after.get(i) != before[i]
        }
        self.ok(
            "tab-stable",
            not changed,
            "switching tabs",
            "every tab keeps its box",
            changed,
        )

    def main_focus(self):
        return (self.win.screen("main-window") or {}).get("focus")

    def focus_return_checks(self):
        """Esc closes the popover and the focus is back on the control that had it when the popover opened."""
        pop = self.record.get("popover")
        if not pop:
            return
        if pop.get("opener_id") is None and pop.get("opener_pane"):
            self.pane_focus_return()
            return
        self.close()
        self.win.key("ctrl+u")  # clears any line typed to open the popover
        self.win.away(self.rest_point())
        # The pane has the keyboard: Ctrl+F6 gives it to the first control.
        self.win.key("ctrl+F6")
        before = self.main_focus()
        if before is None:
            self.ok(
                "focus-return",
                False,
                "Esc",
                "a main-window control focused before opening",
                "none",
            )
            return
        if not self.open():
            return
        self.win.key("Escape")
        try:
            until(lambda: not self.is_open(), "", 2)
        except Fail:
            pass
        after = self.main_focus()
        self.ok(
            "focus-return",
            after == before,
            "Esc",
            f"the focus back on control {before}",
            f"control {after}",
        )

    def pane_focus_return(self):
        """A popover opened by a press in a pane, which takes the keyboard before the popover opens: Esc gives the
        keyboard back to that pane, so a command typed after it runs there (its line and its output both show). A
        popover that cannot be opened for the check fails it: a window that lost the keyboard on an earlier Esc no
        longer opens its popovers from a key."""
        if not self.is_open() and not self.reopen():
            self.ok(
                "focus-return",
                False,
                "Esc",
                "the popover opens again for the check",
                "it did not open",
            )
            return
        pane = self.current()["popover"]["opener_pane"]
        self.win.key("Escape")
        try:
            until(lambda: not self.is_open(), "", 2)
        except Fail:
            pass
        mark = f"zq-back-{self.s:g}".replace(".", "-")
        self.win.type(f"echo {mark}")
        self.win.key("Return")
        try:
            until(lambda: self.win.pane_text(pane).count(mark) >= 2, "", 4)
            got = True
        except Fail:
            got = False
        self.ok(
            "focus-return",
            got,
            "Esc",
            f"typing reaches the opener pane {pane}",
            got,
        )

    def tab_inside_checks(self):
        """While a popover is open Tab moves among its own controls and never to the window behind."""
        if (
            not self.record.get("popover")
            or self.spec.get("tab_completes")
            or not self.reopen()
        ):
            return
        ids = {c["id"] for c in self.controls() if c["enabled"]}
        behind = self.main_focus()
        for _ in range(len(ids) + 2):
            self.win.key("Tab")
            time.sleep(0.15)
            focus = (self.current() or {}).get("focus")
            self.ok(
                "tab-inside",
                focus in ids and self.main_focus() == behind,
                "Tab while open",
                f"focus on one of the popover's controls {sorted(ids)}, the window behind unchanged",
                f"popover focus {focus}, behind {self.main_focus()}",
            )

    def tab_complete_checks(self):
        """A completion list's Tab, with no row holding the keyboard, puts the highlighted row in and closes the list;
        the window behind keeps its focus, and Enter then runs the completed line in the opener's pane."""
        if not self.spec.get("tab_completes"):
            return
        self.close()
        self.win.key("ctrl+u")
        self.clear_focus()
        if not self.open():
            return
        pop = self.current()["popover"]
        first = next((c for c in self.controls() if c.get("checked")), None)
        behind = self.main_focus()
        self.win.key("Tab")
        try:
            until(lambda: not self.is_open(), "", 2)
        except Fail:
            pass
        self.ok(
            "tab-completes",
            not self.is_open() and self.main_focus() == behind,
            "Tab with the list open",
            f"the list closes, the window behind keeps focus {behind}",
            f"open {self.is_open()}, focus {(self.current() or {}).get('focus')}, behind {self.main_focus()}",
        )
        pane = pop.get("opener_pane")
        if pane is None or first is None or self.is_open():
            return
        before = self.win.pane_text(pane).count(first["label"])
        self.win.key("Return")
        try:
            until(
                lambda: self.win.pane_text(pane).count(first["label"]) >= before + 2,
                "",
                4,
            )
            ran = True
        except Fail:
            ran = False
        self.ok(
            "tab-completes",
            ran,
            "Enter after Tab",
            f"runs the line with {first['label']!r} in pane {pane} (the command and its output)",
            ran,
        )

    def filter_checks(self):
        """Typing narrows the list to the rows whose label contains the text, case folded."""
        if not self.spec.get("filter") or not self.reopen():
            return
        labels = [c["label"] for c in self.controls()]
        prefix = next(
            (
                p
                for p in (label[:2].lower() for label in labels)
                if 0 < sum(p in label.lower() for label in labels) < len(labels)
            ),
            None,
        )
        if prefix is None:
            self.ok(
                "filter",
                False,
                "a prefix some rows share and others do not",
                "one",
                labels,
            )
            return
        self.win.type(prefix)
        want = [label for label in labels if prefix in label.lower()]
        if self.spec.get("filter") == "ranked":
            want = ranked_matches(labels, prefix)
        try:
            until(lambda: [c["label"] for c in self.controls()] == want, "", 3)
        except Fail:
            pass
        got = [c["label"] for c in self.controls()]
        if self.spec.get("run_row"):
            # The last row offers to run what was typed in the pane: "Run “ne” in this pane".
            want = want + [f"Run \u201c{prefix}\u201d in …"]
            if got and got[-1].startswith(f"Run \u201c{prefix}\u201d in "):
                got = got[:-1] + [want[-1]]
        self.ok("filter", got == want, f"typed {prefix!r}", want, got)

    def idle_checks(self):
        """No frame is drawn while the popover stays open and nothing happens."""
        if not self.spec.get("idle") or not self.reopen():
            return
        time.sleep(1.0)
        first = self.win.frame_count()
        time.sleep(2.5)
        drawn = self.win.frame_count() - first
        self.ok(
            "idle-frames",
            drawn == 0,
            "popover open, nothing touched for 2.5 s",
            "0 frames",
            f"{drawn} frames",
        )

    def find_checks(self):
        """The find bar: opens, takes the text, marks the current match, Enter goes on to the next, Esc closes."""
        if not self.spec.get("find"):
            return
        self.close()
        # The pane takes the keyboard back from whatever an earlier check left it on, as a click in it does.
        self.win.move(*self.win.menu_point())
        self.win.x("click", "1")
        time.sleep(0.3)
        self.win.type("echo g1 g1 g1")
        self.win.key("Return")
        time.sleep(0.5)
        if not self.open():
            self.ok(
                "find-open",
                False,
                "Ctrl+F",
                "the find bar's controls in the record",
                "no record",
            )
            return
        self.ok(
            "find-open",
            any(c["kind"] == "field" for c in self.controls()),
            "Ctrl+F",
            "a find field",
            "none",
        )
        self.win.type("g1")
        try:
            until(
                lambda: (self.control_by_kind("field") or {}).get("label") == "g1",
                "",
                3,
            )
        except Fail:
            pass
        field = self.control_by_kind("field") or {}
        self.ok(
            "find-type",
            field.get("label") == "g1",
            "typed g1",
            "the field reads g1",
            field.get("label"),
        )
        first = self.match_box()
        panes = self.win.landmarks().get("pane", [])
        self.ok(
            "find-match",
            bool(first) and any(inside(p, first[0], first[1]) for p in panes),
            "a match for g1",
            "a match box inside a pane",
            first,
        )
        total = self.win.text_count("g1")
        self.ok(
            "find-count",
            self.landmark_text("find-count") == count_label("g1", total),
            "g1 typed",
            f"{count_label('g1', total)!r}, the count of g1 in the pane's text from the server",
            repr(self.landmark_text("find-count")),
        )
        if first:
            a = self.win.shot(f"{self.name}-find-match")
            got = grey(a, first[0] + 1, first[1] + 1)
            want = grey_of(EXPECTED, "selection")
            self.ok(
                "find-highlight",
                near(got, want),
                "the current match's top left cell corner",
                f"the selection colour, grey {want}",
                got,
            )
        self.win.key("Return")
        time.sleep(0.4)
        second = self.match_box()
        self.ok(
            "find-next",
            bool(first) and bool(second) and second != first,
            "Enter",
            "the next match, a different box",
            f"{first} then {second}",
        )
        self.win.key("shift+Return")
        time.sleep(0.4)
        back = self.match_box()
        self.ok(
            "find-previous",
            bool(first) and back == first,
            "Shift+Enter after Enter",
            f"the first match again, {first}",
            back,
        )
        self.win.key("Escape")
        try:
            until(lambda: not self.is_open(), "", 2)
        except Fail:
            pass
        self.ok(
            "find-close",
            not self.is_open() and not self.match_box(),
            "Esc",
            "the bar closed and no match marked",
            f"open {self.is_open()}, match {self.match_box()}",
        )

    def landmark_text(self, name):
        """The words of the last line written for a landmark that carries text."""
        lines = [
            r
            for r in self.win.records()
            if r.get("record") == "landmark" and r.get("name") == name
        ]
        return lines[-1].get("text") if lines else None

    def control_by_kind(self, kind):
        return next((c for c in self.controls() if c["kind"] == kind), None)

    def match_box(self):
        found = self.win.landmarks().get("find-match")
        box = found[0] if found else None
        return box if box and box[2] > 0 else None

    def flip_checks(self):
        if not self.spec.get("flip"):
            return
        self.close()
        # In the lower pane's output, above its command bar: the menu has no room right of it or below it.
        at = (self.win.pw - int(30 * self.s), int(0.8 * self.win.ph))
        r = self.open(at=at)
        if not r or not r.get("popover"):
            self.ok(
                "popover", False, f"opened at {at}", "a popover", r and r.get("popover")
            )
            return
        b, opener = r["popover"]["box"], r["popover"]["opener"]
        on = (
            b[0] >= 0
            and b[1] >= 0
            and b[0] + b[2] <= self.win.pw
            and b[1] + b[3] <= self.win.ph
        )
        self.ok(
            "popover",
            on,
            f"opened by the bottom-right corner at {at}",
            f"inside the window 0,0 {self.win.pw}x{self.win.ph}",
            b,
        )
        self.ok(
            "popover",
            not covers(b, opener),
            f"opened by the bottom-right corner at {at}",
            f"clear of its opener {opener}",
            b,
        )
        self.close()

    # -- G4: pages that scroll, and the config file ------------------------------------------------------------

    def wheel(self, notches):
        """Turns the wheel over the Settings body; returns the scroll position it ends at."""
        panel = self.win.landmarks()["settings-panel"][0]
        self.win.move(panel[0] + panel[2] // 2, panel[1] + panel[3] * 3 // 4)
        before = (self.current() or {}).get("scroll", {}).get("top")
        for _ in range(abs(notches)):
            self.win.x("click", "5" if notches > 0 else "4")
        try:
            until(
                lambda: (self.current() or {}).get("scroll", {}).get("top") != before,
                "",
                1.5,
            )
        except Fail:
            pass
        return (self.current() or {}).get("scroll", {}).get("top")

    def scroll_checks(self):
        """A long page: scroll it down notch by notch; what comes into view is checked like the first screenful
        (record, resting colour, hover), and the last control must come into view."""
        if not self.spec.get("scroll") or not self.reopen():
            return
        total = len(self.all_at_open)
        seen = {c["id"] for c in self.controls()}
        for _ in range(40):
            scroll = (self.current() or {}).get("scroll") or {}
            if scroll.get("top", 0) >= scroll.get("max", 0):
                break
            top = self.wheel(2)
            self.ok(
                "scroll",
                top is not None and top > scroll["top"],
                "the wheel over the page body",
                "scrolls it",
                f"{scroll['top']} then {top}",
            )
            fresh = [c for c in self.controls() if c["id"] not in seen]
            if fresh:
                seen |= {c["id"] for c in fresh}
                self.record = self.in_view(self.current())
                for step in (self.static_checks, self.rest_checks, self.hover_checks):
                    step()
        self.ok(
            "scroll",
            len(seen) == total,
            "scrolling to the end of the page",
            f"all {total} controls came into view",
            f"{len(seen)} did",
        )
        self.wheel(-60)

    def bring_into_view(self, label, kind, group=None):
        """The control of a row, scrolled into the body (the page back to its top first). Option labels such as
        "On" repeat from row to row, so a segment is found by its row's group as well."""

        def find():
            r = self.current() or {}
            return next(
                (
                    c
                    for c in r.get("controls", [])
                    if c["label"] == label
                    and c["kind"] == kind
                    and (group is None or c["group"] == group)
                ),
                None,
            )

        self.wheel(-60)
        for _ in range(40):
            control = find()
            if control is None or control.get("visible", True):
                return control
            self.wheel(2)
        return find()

    def roundtrip_checks(self):
        """Every row of the page: change its control through the record, read the config file, compare with the
        value rows.json expects; then see the window show the stored value, and see an edit of the file reach the
        window. The values come from rows.json (the answers file's key list), never from the program."""
        if not self.spec.get("roundtrip"):
            return
        import config_model

        rows = [r for r in config_model.ROWS if r["page"] == self.spec["roundtrip"]]
        if not rows or not self.reopen():
            return
        path = getattr(self.win, "config_path", None)
        if path is None:
            self.ok(
                "config-roundtrip",
                False,
                "the config file",
                "the window's config path",
                "none",
            )
            return
        # Every setting starts from the value rows.json gives; the window takes the file up (live reload).
        config_model.write_config(path, config_model.initial_config())
        first = next((r for r in rows if r["control"] == "switch"), None)
        if first is not None:
            try:
                until(
                    lambda: (
                        (
                            self.control(
                                (
                                    self.bring_into_view(first["label"], "switch") or {}
                                ).get("id")
                            )
                            or {}
                        ).get("checked")
                        == first["initial"]
                        or None
                    ),
                    "",
                    4,
                )
            except Fail:
                pass
        for row in rows:
            self.roundtrip_row(row, path, config_model)
        self.file_edit_reload(rows, path, config_model)

    def roundtrip_row(self, row, path, config_model):
        kind = {
            "switch": "switch",
            "segments": "segment",
            "field": "field",
            "slider": "slider",
        }[row["control"]]
        label = row["set"] if kind == "segment" else row["label"]
        control = self.bring_into_view(
            label, kind, f"row:{row['key']}" if kind == "segment" else None
        )
        if control is None:
            self.ok(
                "config-roundtrip",
                False,
                row["label"],
                f"a {kind} control",
                "not in the record",
            )
            return
        self.win.move(*centre(control["box"]))
        tolerance = 1e-6
        if kind == "field":
            self.win.x("mousedown", "1")
            self.win.x("mouseup", "1")
            self.win.key("ctrl+a")
            self.win.type(str(row["set"]))
            self.win.key("Return")
        elif kind == "slider":
            x, y, w, h = control["box"]
            # A click lands on a whole device pixel, so the value can be off by half a pixel's worth of the range.
            tolerance = (control["max"] - control["min"]) / max(w - 1, 1) / 2 + 1e-6
            share = (row["set"] - control["min"]) / (control["max"] - control["min"])
            self.win.move(x + round(share * (w - 1)), y + h // 2)
            self.win.x("mousedown", "1")
            self.win.x("mouseup", "1")
        else:
            self.win.x("mousedown", "1")
            self.win.x("mouseup", "1")
        got = None
        try:
            until(
                lambda: (
                    values_match(
                        config_model.read_config(path).get(row["key"]),
                        row["expect"],
                        tolerance,
                    )
                    or None
                ),
                "",
                2.5,
            )
        except Fail:
            pass
        got = config_model.read_config(path).get(row["key"], "missing")
        self.ok(
            "config-roundtrip",
            values_match(got, row["expect"], tolerance),
            f"{row['label']} ({row['key']}) set to {row['set']}",
            f"config holds {row['expect']!r}",
            f"{got!r}",
        )
        shown = self.control(control["id"]) or {}
        shown_value = {
            "switch": shown.get("checked"),
            "segment": shown.get("checked"),
            "field": shown.get("field_text"),
            "slider": shown.get("value"),
        }[kind]
        want = {
            "switch": row["expect"],
            "segment": True,
            "field": str(row["set"]),
            "slider": row["expect"],
        }[kind]
        self.ok(
            "config-live",
            values_match(shown_value, want, tolerance),
            f"{row['label']} after the change",
            f"the window shows {want!r}",
            f"{shown_value!r}",
        )

    def file_edit_reload(self, rows, path, config_model):
        """A change to the config file made from outside is seen in the window within a few seconds."""
        row = next((r for r in rows if r["control"] == "switch"), None)
        if row is None:
            return
        values = config_model.read_config(path)
        values[row["key"]] = not values[row["key"]]
        config_model.write_config(path, values)
        control = (
            self.control(self.bring_into_view(row["label"], "switch")["id"])
            if self.bring_into_view(row["label"], "switch")
            else None
        )
        try:
            until(
                lambda: (
                    (self.control(control["id"]) or {}).get("checked")
                    == values[row["key"]]
                    or None
                ),
                "",
                4,
            )
        except (Fail, TypeError):
            pass
        shown = (self.control(control["id"]) or {}).get("checked") if control else None
        self.ok(
            "config-live",
            shown == values[row["key"]],
            f"{row['label']} after the file was edited",
            f"the window shows {values[row['key']]!r}",
            f"{shown!r}",
        )

    def run(self):
        if not self.open():
            self.ok(
                "recorded",
                False,
                "controls",
                "drawn by the control set and in its record",
                "no record",
            )
            return
        steps = [
            self.static_checks,
            self.rest_checks,
            self.hover_checks,
            self.press_checks,
            self.tab_checks,
            self.arrow_checks,
            self.switch_checks,
            self.fire_checks,
            self.escape_checks,
            self.flip_checks,
            self.focus_return_checks,
            self.tab_inside_checks,
            self.tab_complete_checks,
            self.filter_checks,
            self.idle_checks,
            self.find_checks,
            self.click_focus_checks,
            self.tab_stable_checks,
            self.scroll_checks,
            self.roundtrip_checks,
        ]
        if FAULT:  # a planted fault needs only the check aimed at it, and its neighbours for the screen's state
            steps = {
                "border-2px": [self.tab_checks],
                "hit-larger": [self.static_checks, self.hover_checks],
                "press-fires": [self.press_checks, self.fire_checks],
                "tab-skip": [self.tab_checks],
                "popover-over-opener": [self.static_checks],
                "esc-loses-focus": [self.focus_return_checks],
                "idle-draw": [self.idle_checks],
                "find-count-off": [self.find_checks],
                "find-mark-accent": [self.find_checks],
                "shift-enter-next": [self.find_checks],
                "tab-traps-completion": [self.tab_complete_checks],
                "click-takes-focus": [self.click_focus_checks],
                "active-tab-grows": [self.tab_stable_checks],
                "selected-segment-hover": [self.hover_checks],
                "palette-no-run": [self.filter_checks],
                "setting-not-written": [self.roundtrip_checks],
                "switch-hit-larger": [self.static_checks, self.hover_checks],
                "page-control-out-of-tab-order": [self.tab_checks],
            }[FAULT]
        for step in steps:
            try:
                step()
            except Fail as e:
                self.ok(
                    step.__name__.replace("_checks", ""),
                    False,
                    "setup",
                    "the screen answers",
                    str(e),
                )
            if self.spec["close"] is None:
                continue
            if (
                step not in (self.static_checks, self.rest_checks)
                and not self.is_open()
                and step != self.flip_checks
            ):
                self.open()
        self.close()


def make_window(scale):
    if SIMULATE and SETTINGS:
        from simulated_settings import SimSettingsWindow

        return SimSettingsWindow(scale, THEME, FAULT)
    if SIMULATE and FLOATING:
        from simulated_floating import SimFloatingWindow

        return SimFloatingWindow(scale, THEME, FAULT)
    if SIMULATE:
        return SimWindow(scale, THEME, FAULT)
    return RealWindow(scale, WORK, APP, CONFIG, FAULT, KEEP)


def main():
    if FAULT and FAULT not in FAULTS:
        print(f"FAIL: setup unknown fault {FAULT}")
        FAILED.append("setup")
        return
    names = arg("--screens", ",".join(SCREENS)).split(",")
    unknown = [n for n in names if n not in SCREENS]
    if unknown:
        print(f"FAIL: setup unknown screens {unknown}")
        FAILED.append("setup")
        return
    if not SIMULATE:
        for tool in ("Xvfb", "xdotool", "ffmpeg"):
            if not shutil.which(tool):
                FAILED.append(f"FAIL: setup {tool} missing")
                print(FAILED[-1])
                return
        if WORK.exists():
            shutil.rmtree(WORK)
        WORK.mkdir(mode=0o700, parents=True)
    try:
        for s in SCALES:
            use_expected(s, THEME)
            win = make_window(s)
            try:
                for name in names:
                    Screen(win, name, SCREENS[name]).run()
            finally:
                win.close()
    finally:
        if not SIMULATE and not KEEP:
            shutil.rmtree(WORK, ignore_errors=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # a crash still reports FAIL, never PASS
        FAILED.append(f"FAIL: gate crashed {type(e).__name__}: {e}")
        print(FAILED[-1])
    for (name, screen, scale), n in sorted(PASSED.items()):
        if not any(
            line.startswith(f"FAIL: {name} {screen} @{scale:g} ") for line in FAILED
        ):
            print(f"ok: {name} {screen} @{scale:g} ({n} checked)")
    if not FAILED and PASSED:
        print("PASS: controls")
        sys.exit(0)
    sys.exit(1)
