#!/usr/bin/env python3
"""Design token gate for clanky-gpui's window chrome: every interface size, radius, colour and text size comes from one
token module, and the window drawn on screen matches the rulebook's table.

Usage: gate-design-tokens.py [--sources PATH] [--scales 1] [--theme graphite-dark] [--app PATH] [--simulate]
                             [--fault NAME] [--keep]
Prints one line per check ("ok: ..." or "FAIL: <check> <expected> <observed>"), then "PASS: design tokens" when every
check passed. Exit status 0 only on PASS.

Retargeted from the calm window's gate (design-tokens branch). Every expected value comes from expected.py (the
rulebook's table and the Inter font files); nothing from the program under test.

Check 1, no loose numbers. In crates/clanky-gpui/src/*.rs outside tokens.rs (or the PATH given by --sources), outside
comments and test modules, a number literal given as a size, spacing, radius or text size, or a colour literal, is a
hit. gpui-ce builder calls count: .h(px(..)) .w .min_h .max_w .size .top .left .inset (size); .p .px .py .pt .m .mx
.mt .gap .gap_x (spacing); .rounded (radius, also rounded_sm and rounded_md); .text_size (text); the numbered forms
.p_2 .gap_4 .h_8; rgb(0x..) rgba(0x..) hsla(..) and "#rrggbb" (colour). A literal 0 is not a design value. Layout
arithmetic (.w(px(x / 2.0))) is not counted. A `// gate: allow <reason>` comment on the line exempts it; every
exemption is listed in the output. Expected: 0 hits.

Check 2, pixels match the table, on one window at each scale (default 1; 1200 x 800 logical px), read from the control
record's controls and landmarks (RECORD.md) and from screenshots; the tab names come from the scratch HOME (a folder
named HIH, so the active tab's label is "HIH", all flat-topped capitals):
  title-bar-height   40 +-1   the top of the first pane's edge in the pixels, less the pane gap
  title-bar-landmark 40 +-1   the record's title-bar box
  tab-height         28 +-1   the active tab's fill, edge rows counted by their border
  tab-radius          8 +-1   its corners, from the area missing in each corner
  status-height      32 +-1   rows from the bottom to the line above the status line, found in the pixels
  pane-radius        14 +-1   a circle fitted to the first pane's top-left corner
  pane-border-alpha   9 %     the pane's left edge pixel against the rulebook's border over background, +-3 levels
  pane-gap, pane-margin 8 +-1 between the panes, and from the window's edge to the first pane
  text-ui-cap-height, text-status-cap-height, text-meta-cap-height  +-0.5 px, ink height of "HIH" in the active tab,
                     of the count digit in the status line and of the block meta text, against Inter's H
  selection-colour   a drag over the pane's first text row fills with the theme's selection colour, +-3 levels
The tab, status and meta heights and the selection need the landmarks of RECORD.md; a missing one fails its check.

Planted faults (--simulate --fault NAME; each must fail the check named):
  tab-too-tall    the tabs 2 px too tall (simulated window)           -> tab-height
  loose-literal   a `.rounded(px(9.))` in fixtures/loose-literal.rs  -> loose-numbers
"""

import os
import re
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "floating"))
sys.path.insert(0, str(HERE.parent / "settings"))
import chrome_pixels as pixels  # noqa: E402
import expected as expected_values  # noqa: E402
import expected_floating  # noqa: E402
import expected_settings  # noqa: E402
from chrome_harness import Fail, RealWindow, until  # noqa: E402
from simulated_window import SimWindow  # noqa: E402

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


FAULT = arg("--fault")
FAULTS = {
    "tab-too-tall": "tab-height",
    "loose-literal": "loose-numbers",
    "pane-rounded": "pane-radius",  # the panes drawn with the old 14 px corners
    "palette-row-short": "palette-row-height",  # G3: a palette row 2 px short (--floating)
    "palette-text-big": "palette-row-cap-height",  # G3: palette row text 2 px too large (--floating)
    "find-field-short": "find-bar-field-height",  # G3: the find bar's field 2 px short (--floating)
    "card-padding": "settings-card-padding-x",  # G4: every card's text 2 px too far right (--settings)
}
SIMULATE = "--simulate" in ARGS
if (
    SIMULATE
):  # a simulated window answers at once: pauses move a clock, they do not wait
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import virtual_clock

    virtual_clock.install()
FLOATING = "--floating" in ARGS  # G3: also measure the floating screens
# --screens a,b measures only those floating screens, for a part that brings one screen before the rest exist.
FLOATING_ONLY = set(arg("--screens", "").split(",")) - {""}
SETTINGS = "--settings" in ARGS  # G4: also measure the Settings window
KEEP = "--keep" in ARGS
SCALES = [float(v) for v in arg("--scales", "1").split(",")]
THEME = arg("--theme", "graphite-dark")
REPO = HERE.parents[1]
SOURCES = Path(
    arg(
        "--sources",
        HERE / "fixtures/loose-literal.rs"
        if FAULT == "loose-literal"
        else REPO / "crates/clanky-gpui/src",
    )
)
APP = Path(
    arg("--app", os.environ.get("CLANKY_GPUI_BIN", REPO / "target/release/clanky-gpui"))
)
WORK = Path(
    os.environ.get("CLANKY_DESIGN_WORK", "/srv/devops/build/clanky-design-gate-run")
)
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
    # The agent picker's tools: two found on PATH and one that is not installed. Fake Tool, which the controls gate
    # fires, ends the shell of the pane it starts in, so the pane closes again and the window keeps one wide pane.
    agents=[
        dict(name="Fake Tool", command="sh -c 'kill -HUP $PPID'"),
        dict(name="Other Tool", command="true"),
        dict(name="Missing Tool", command="no-such-agent-t9"),
    ],
)
RESULTS = []  # (check, ok, expected, observed)


def record(check, ok, expected, observed):
    RESULTS.append((check, ok, expected, observed))
    print(
        f"ok: {check} {expected} {observed}"
        if ok
        else f"FAIL: {check} {expected} {observed}",
        flush=True,
    )


# ---------------------------------------------------------------- check 1: no loose numbers

NUM = r"(?:\d+\.\d*|\.\d+|\d+)(?:_?f32|_?f64)?"
ALLOW = re.compile(r"//\s*gate:\s*allow\s+(\S.*)$")
CALLS = {
    "size": (
        "h",
        "w",
        "min_h",
        "min_w",
        "max_h",
        "max_w",
        "size",
        "top",
        "left",
        "right",
        "bottom",
        "inset",
    ),
    "spacing": (
        "p",
        "px",
        "py",
        "pt",
        "pb",
        "pl",
        "pr",
        "m",
        "mx",
        "my",
        "mt",
        "mb",
        "ml",
        "mr",
        "gap",
        "gap_x",
        "gap_y",
    ),
    "radius": ("rounded", "rounded_t", "rounded_b", "rounded_l", "rounded_r"),
    "text": ("text_size",),
}
NUMBERED = re.compile(
    r"\.(p|px|py|pt|pb|pl|pr|m|mx|my|mt|mb|ml|mr|gap|gap_x|gap_y|h|w|size|min_h|min_w|max_h|max_w)_(\d+(?:_\d+)?)\("
)
NAMED_RADIUS = re.compile(r"\.rounded_(?:xs|sm|md|lg|xl|2xl|3xl)\(")
COLOUR = (
    re.compile(r"\b(?:rgb|rgba)\(\s*0x[0-9a-fA-F]+"),
    re.compile(r"\bhsla\(\s*" + NUM),
    re.compile(r'"#[0-9a-fA-F]{6}(?:[0-9a-fA-F]{2})?"'),
)


def source_files():
    if SOURCES.is_file():
        return [SOURCES]
    return sorted(p for p in SOURCES.rglob("*.rs") if p.name != "tokens.rs")


def kind_of_numbered(name):
    base = name
    for kind in ("size", "spacing"):
        if base in CALLS[kind]:
            return kind
    return "size"


def scan(texts):
    """Loose numbers in {path: source text}: (hits, exemptions), each a list of (path, line, kind, snippet or reason)."""
    hits, allowed = set(), []
    for path, text in texts.items():
        clean = strip_tests(strip_comments(text))
        original = text.splitlines()
        found = set()
        for kind, names in CALLS.items():
            pattern = re.compile(
                r"\.("
                + "|".join(names)
                + r")\(\s*(?:(?:px|rems|rem)\(\s*)?("
                + NUM
                + r")(?![\w.])"
            )
            for m in pattern.finditer(clean):
                if literal(m.group(2)):
                    found.add((clean.count("\n", 0, m.start()) + 1, kind))
        for m in NUMBERED.finditer(clean):
            if not literal(m.group(2).replace("_", ".")):
                continue  # .min_w_0(): a literal 0 is not a design value
            found.add(
                (clean.count("\n", 0, m.start()) + 1, kind_of_numbered(m.group(1)))
            )
        for m in NAMED_RADIUS.finditer(clean):
            found.add((clean.count("\n", 0, m.start()) + 1, "radius"))
        for pattern in COLOUR:
            for m in pattern.finditer(clean):
                found.add((clean.count("\n", 0, m.start()) + 1, "colour"))
        lines_with_hits = {line for line, _ in found}
        for m in re.finditer(r"\bpx\(\s*(" + NUM + r")\s*\)", clean):
            line = clean.count("\n", 0, m.start()) + 1
            if line not in lines_with_hits and literal(m.group(1)):
                found.add((line, "size"))
        for line, kind in found:
            comment = ALLOW.search(original[line - 1])
            if comment:
                allowed.append((path, line, kind, comment.group(1).strip()))
            else:
                hits.add((path, line, kind, original[line - 1].strip()[:110]))
    return sorted(hits), sorted(allowed)


def check_sources():
    files = source_files()
    if not files:
        record(
            "loose-numbers", False, "sources to scan", f"no .rs files under {SOURCES}"
        )
        return
    texts = {
        str(p.relative_to(REPO) if REPO in p.parents else p): p.read_text()
        for p in files
    }
    hits, allowed = scan(texts)
    for h in hits[:12]:
        print(f"  {h[0]}:{h[1]} {h[2]}: {h[3]}")
    if len(hits) > 12:
        print(f"  ... and {len(hits) - 12} more")
    for a in allowed:
        print(f"  allowed {a[0]}:{a[1]} {a[2]}: {a[3]}")
    by_kind = {
        k: sum(1 for h in hits if h[2] == k)
        for k in ("size", "spacing", "radius", "text", "colour")
    }
    summary = ", ".join(f"{v} {k}" for k, v in by_kind.items())
    record(
        "loose-numbers",
        not hits,
        "0",
        f"{len(hits)} ({summary}); {len(allowed)} allowed",
    )


def blank(text):
    return re.sub(r"[^\n]", " ", text)


def skip_string(src, i):
    """Index just past the string or character literal starting at i (a lifetime counts as one character)."""
    if src[i] == '"':
        j = i + 1
        while j < len(src) and src[j] != '"':
            j += 2 if src[j] == "\\" else 1
        return j + 1
    m = re.match(r"'(\\.|[^\\'])'", src[i:])
    return i + (len(m.group(0)) if m else 1)


def strip_comments(src):
    out, i, n = [], 0, len(src)
    while i < n:
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(blank(src[i:j]))
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append(blank(src[i:j]))
            i = j
        elif src[i] in "\"'":
            j = skip_string(src, i)
            out.append(src[i:j])
            i = j
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def matching(src, i):
    """Index of the bracket closing the one at i, skipping string literals."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack, j = [], i
    while j < len(src):
        c = src[j]
        if c in "\"'":
            j = skip_string(src, j)
            continue
        if c in pairs:
            stack.append(pairs[c])
        elif stack and c == stack[-1]:
            stack.pop()
            if not stack:
                return j
        j += 1
    return len(src) - 1


def strip_tests(src):
    """Blanks every item marked #[cfg(test)]: test fixtures are not interface values."""
    while True:
        k = src.find("#[cfg(test)]")
        if k < 0:
            return src
        b = src.find("{", k)
        semi = src.find(";", k)
        end = semi if 0 <= semi < b or b < 0 else matching(src, b)
        src = src[:k] + blank(src[k : end + 1]) + src[end + 1 :]


def split_args(text):
    out, depth, cur, i = [], 0, [], 0
    while i < len(text):
        c = text[i]
        if c in "\"'":
            j = skip_string(text, i)
            cur.append(text[i:j])
            i = j
            continue
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        if c == "," and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    out.append("".join(cur))
    return [a.strip() for a in out]


def literal(arg):
    """The number an argument starts with, when it is a literal (possibly inside parentheses), else None."""
    m = re.match(r"[(\s]*(" + NUM + r")(?![\w.])", arg)
    return float(re.sub(r"_?f(32|64)$", "", m.group(1))) if m else None


# ---------------------------------------------------------------- check 2: the window

PIXEL_TOLERANCE = 3  # colour levels, from expected.py


def text_check(check, want, observed, tolerance):
    if observed is None:
        record(check, False, f"{want:.2f}", "no ink measured")
    else:
        record(
            check, abs(observed - want) <= tolerance, f"{want:.2f}", f"{observed:.2f}"
        )


def guarded(check, fn):
    """Runs one measurement; a failure to find what it measures is that check's FAIL, and the gate goes on."""
    try:
        fn()
    except Exception as e:  # noqa: BLE001 - every failure is reported as this check's result
        record(
            check,
            False,
            "measurable",
            f"{type(e).__name__}: {str(e)[:160]}".replace("\n", " "),
        )


def landmark(win, name, index=0):
    found = win.landmarks().get(name)
    if not found or index >= len(found):
        raise Fail(f"no {name} landmark in the record")
    return found[index]


def edge_tolerance(e):
    return e["chrome"]["tab_height"]["tolerance_px"]


def pane_gap_margin(e, a, pane, frame, panes):
    """The pane gap and the window margin, from the pixels along the pane's middle row. The margin is read on the right
    of the rightmost pane, where the window's edge follows it: on the left the tools panel, in the same frame colour,
    may lie between a pane and the edge."""
    y_mid = pane[1] + pane[3] // 2
    last = max(panes, key=lambda box: box[0] + box[2]) if panes else pane
    right = last[0] + last[2]
    edge = right
    while edge < a.shape[1] and pixels.cdist(a[y_mid, edge], frame) <= 2:
        edge += 1
    margin = edge - right if edge == a.shape[1] else None
    x = pane[0] + pane[2] // 2
    while pixels.cdist(a[y_mid, x], frame) > 2 and x < a.shape[1] - 1:
        x += 1
    gap_start = x
    while pixels.cdist(a[y_mid, x], frame) <= 2 and x < a.shape[1] - 1:
        x += 1
    return margin, x - gap_start


def run_first_command(win):
    """One command in the first pane, so that a finished block (and its meta row) is on screen before the checks."""
    win.type("echo done")
    win.key("Return")
    until(
        lambda: win.landmarks().get("meta-text"),
        "the meta-text landmark after one command",
        15,
    )
    win.away()
    time.sleep(0.5)


def window_checks(win, e):
    run_first_command(win)
    a = win.shot("main")
    frame = tuple(int(v) for v in a[2, a.shape[1] // 2])
    tol = edge_tolerance(e)
    chrome = e["chrome"]
    pane = landmark(win, "pane", 0)
    # The title bar has the frame colour like the backdrop under it, so its height is the first pane's top less the gap.
    top = pane[1]
    while top > 0 and pixels.cdist(a[top - 1, pane[0] + pane[2] // 2], frame) > 2:
        top -= 1
    gap = chrome["pane_gap"]["device"]

    def title_bar():
        pane_top = next(
            y
            for y in range(a.shape[0])
            if pixels.cdist(a[y, pane[0] + pane[2] // 2], frame) > 2
        )
        bar = pane_top - gap
        record(
            "title-bar-height",
            abs(bar - chrome["title_bar_height"]["device"]) <= tol,
            chrome["title_bar_height"]["device"],
            f"{bar:g}",
        )
        box = landmark(win, "title-bar")
        record(
            "title-bar-landmark",
            abs(box[3] - chrome["title_bar_height"]["device"]) <= tol,
            chrome["title_bar_height"]["device"],
            box[3],
        )

    guarded("title-bar-height", title_bar)

    def tab():
        active = next(
            c
            for c in win.screen("main-window")["controls"]
            if c["kind"] == "tab" and c["checked"]
        )
        x, y, w, h = active["box"]
        container = tuple(a[y + h // 2, max(x - 6, 0)])
        container_grey = round(sum(container) / 3)
        fill = round(
            pixels.blend(
                container_grey,
                e_grey(e, "foreground"),
                expected_values.load_tokens()["active_tab"]["fill_foreground_alpha"][
                    "value"
                ],
            )
        )
        ring = pixels.blend(
            fill,
            e_grey(e, "foreground"),
            expected_values.load_tokens()["active_tab"]["inner_ring_foreground_alpha"][
                "value"
            ],
        )
        height = pixels.fill_height(
            a,
            active["box"],
            fill,
            e["controls"]["tab"]["radius"]["device"],
            ring,
            PIXEL_TOLERANCE,
        )
        want = e["controls"]["tab"]["height"]["device"]
        record("tab-height", abs(height - want) <= tol, f"{want:g}", height)
        radius = pixels.corner_area_radius(
            a,
            [x, y, w, h],
            fill,
            container_grey,
            e["controls"]["tab"]["radius"]["device"],
        )
        want = e["controls"]["tab"]["radius"]["device"]
        record("tab-radius", abs(radius - want) <= tol, f"{want:g}", f"{radius:.2f}")
        label = active["label_box"]
        text_check(
            "text-ui-cap-height",
            e["cap_heights"]["text_ui"]["device"],
            pixels.ink_height(
                a, (label[0], y + 3, label[0] + label[2], y + h - 3), (fill, fill, fill)
            ),
            e["cap_heights"]["text_ui"]["tolerance_px"],
        )

    guarded("tab-height", tab)

    def status():
        height_px = a.shape[0]
        x = a.shape[1] // 2
        y = height_px - 1
        while y > 0 and pixels.cdist(a[y - 1, x], frame) <= 2:
            y -= 1
        line = y - 1  # the hairline row above the status line's fill
        height = height_px - line
        want = chrome["status_height"]["device"]
        record("status-height", abs(height - want) <= tol, f"{want:g}", height)
        count = landmark(win, "status-count")
        text_check(
            "text-status-cap-height",
            e["cap_heights"]["text_status"]["device"],
            pixels.coverage_height(
                a,
                (count[0], count[1] + 1, count[0] + count[2], count[1] + count[3] - 1),
                frame,
            ),
            e["cap_heights"]["text_status"]["tolerance_px"],
        )

    guarded("status-height", status)

    def meta():
        box = landmark(win, "meta-text")
        text_check(
            "text-meta-cap-height",
            e["cap_heights"]["text_meta"]["device"],
            pixels.ink_height(
                a,
                (box[0], box[1], box[0] + box[2], box[1] + box[3]),
                e_rgb(e, "background"),
            ),
            e["cap_heights"]["text_meta"]["tolerance_px"],
        )

    guarded("text-meta-cap-height", meta)

    def panes():
        margin, between = pane_gap_margin(
            e, a, pane, frame, win.landmarks().get("pane", [])
        )
        record(
            "pane-margin",
            margin is not None and abs(margin - chrome["pane_margin"]["device"]) <= tol,
            chrome["pane_margin"]["device"],
            margin,
        )
        record(
            "pane-gap",
            abs(between - chrome["pane_gap"]["device"]) <= tol,
            chrome["pane_gap"]["device"],
            between,
        )
        # Flush panes have no border of their own: the divider under the title bar is one device pixel of the
        # border colour over the frame, with the title bar's frame colour above it and the pane's fill below.
        x_mid = pane[0] + pane[2] // 2
        divider = tuple(int(v) for v in a[pane[1] - 1, x_mid])
        want = tuple(
            int(e["colours"]["border_over_frame"]["hex"][i : i + 2], 16)
            for i in (1, 3, 5)
        )
        tolerance = chrome["pane_border_alpha"]["tolerance_levels"]
        one_row = pixels.cdist(a[pane[1] - 2, x_mid], frame) <= 2 and pixels.cdist(a[pane[1], x_mid], a[pane[1] - 1, x_mid]) > 2
        record(
            "pane-border-alpha",
            one_row and all(abs(p - q) <= tolerance for p, q in zip(divider, want)),
            f"{chrome['pane_border_alpha']['value']:.3f} (rgb {want}), one row",
            f"rgb {divider}, {'one row' if one_row else 'not one row'}",
        )
        radius = pane_radius(a, pane, frame, e)
        record(
            "pane-radius",
            abs(radius - chrome["pane_radius"]["device"]) <= tol,
            chrome["pane_radius"]["device"],
            f"{radius:.2f}",
        )

    guarded("pane-gap", panes)
    guarded("selection-colour", lambda: selection_check(win, e))
    if FLOATING:
        floating_checks(win, e)
    if SETTINGS:
        guarded("settings", lambda: settings_checks(win, e))


def e_rgb(e, role):
    return tuple(int(e["colours"][role]["hex"][i : i + 2], 16) for i in (1, 3, 5))


def e_grey(e, role):
    return round(sum(e_rgb(e, role)) / 3)


def pane_radius(a, pane, frame, e):
    """The pane's corner radius. A square corner (the flush panes) is read at the corner pixel itself: 0 when it has
    the pane's fill, else the fill's distance along the top edge from the corner. A rounded one is a circle fitted to
    the top-left corner; the border sits inside the pane's box over the fill, so the fill is fitted (the border row
    and column count as outside it) and the border's width is added back."""
    import numpy as np

    x, y, w, h = pane
    y_mid = y + h // 2
    fill = tuple(int(v) for v in a[y_mid, x + 20])
    if e["chrome"]["pane_radius"]["device"] == 0:
        inset = 0
        while inset < w // 2 and pixels.cdist(a[y, x + inset], fill) > 2:
            inset += 1
        return float(inset)
    k = int(np.argmax([abs(s - o) for s, o in zip(fill, frame)]))
    cov = np.clip((a[..., k] - frame[k]) / (fill[k] - frame[k]), 0, 1)
    radius = e["chrome"]["pane_radius"]["device"]
    border = e["chrome"]["hairline_device_px"]["device"]
    fitted = pixels.circle_fit_radius(
        cov, x + border, y + border, y_mid, x + w // 2, int(3 * radius), radius
    )
    return fitted + border


def selection_check(win, e):
    """A drag over the first text row of the pane: the commonest colour inside the dragged span is the selection fill."""
    row = landmark(win, "pane-text")
    y = row[1] + row[3] // 2
    start, end = row[0] + 4, row[0] + row[2] - 4
    win.move(start, y)
    win.x("mousedown", "1")
    win.move((start + end) // 2, y)
    win.move(end, y)
    win.x("mouseup", "1")
    time.sleep(0.5)
    a = win.shot("selection")
    band = a[row[1] : row[1] + row[3], start + 2 : end - 2].reshape(-1, 3)
    values, counts = __import__("numpy").unique(band, axis=0, return_counts=True)
    observed = tuple(int(v) for v in values[counts.argmax()])
    want = e_rgb(e, "selection")
    tolerance = e["colour_tolerance_levels"]["value"]
    record(
        "selection-colour",
        all(abs(p - q) <= tolerance for p, q in zip(observed, want)),
        e["colours"]["selection"]["hex"],
        f"rgb {observed}",
    )


# ---------------------------------------------------------------- G3: the floating screens

# How each popover opens. The context menu's right-click is the window's own; the palette's (Ctrl+Shift+P) and the
# find bar's (Ctrl+Shift+F, today's: see find_bar_checks) keys are the real ones. TODO(G3): the others are guesses
# until G3's key bindings exist.


def type_completion(win):
    win.type("ls scripts/gate-m")


def open_menu(win):
    x, y = win.menu_point()
    win.move(x, y)
    win.x("click", "3")


FLOATING_SCREENS = {
    "palette": dict(
        open=lambda win: win.key("ctrl+shift+p"),
        row="palette_row",
        input="palette_input",
        footer="palette_footer",
        text=True,
    ),
    # The real key: Ctrl+Shift+G opens the picker with the config's three agent tools.
    "agent-picker": dict(
        open=lambda win: win.key("ctrl+shift+g"),
        row="agent_picker_row",
        input="palette_input",
    ),
    # Today's Tab completion. The line is typed before the closed frame is taken, so the shadow check compares the
    # same input line with and without the list over it.
    "completion-list": dict(
        prepare=type_completion, open=lambda win: win.key("Tab"), row="completion_row"
    ),
    "context-menu": dict(open=open_menu, row="menu_item"),
}
ROW_KINDS = ("palette-row", "menu-item")
HOVER_TOLERANCE = 1  # grey levels: a hovered row's fill is a flat colour


def run_of(a, x, y, grey_want, limit):
    """The run of pixels around (x, y) in column x whose grey is within the tolerance of grey_want: (top, length)."""
    top = y
    while top > 0 and abs(pixels.grey(a, x, top - 1) - grey_want) <= HOVER_TOLERANCE:
        top -= 1
    bottom = y
    while (
        bottom < limit - 1
        and abs(pixels.grey(a, x, bottom + 1) - grey_want) <= HOVER_TOLERANCE
    ):
        bottom += 1
    return top, bottom - top + 1


def hairline_rows(a, box, x, sep_grey):
    """The rows in column x inside the box whose grey is the hairline's."""
    return [
        y
        for y in range(box[1] + 2, box[1] + box[3] - 2)
        if abs(pixels.grey(a, x, y) - sep_grey) <= 2
    ]


def popover_radius(a, box, card_grey, e):
    """The popover's corner radius, from its top-right corner (nothing else is drawn beside that corner): per row near
    the top, how far in the card begins, fitted with a circle."""
    import numpy as np

    radius = e["floating"]["popover_radius"]["device"]
    right = box[0] + box[2]
    points = []
    for dy in range(int(radius) + 2):
        y = box[1] + dy
        outside = pixels.grey(a, right + 4, y)
        middle = (outside + card_grey) / 2
        x = next(
            (
                x
                for x in range(right + 2, right - int(radius) - 6, -1)
                if (pixels.grey(a, x, y) - middle) * (card_grey - outside) >= 0
            ),
            None,
        )
        if x is not None:
            points.append((dy + 0.5, right - 1 - x))
    return min(
        (r / 20 for r in range(10, 20 * 2 * int(radius) + 1)),
        key=lambda r: sum(
            (inset - (r - np.sqrt(max(r * r - (r - dy) ** 2, 0)) if dy < r else 0)) ** 2
            for dy, inset in points
        ),
    )


def shadow_check(win, e, screen, box, closed):
    """Greys below the popover's bottom edge, middle column, against the Gaussian model on the closed window's pixels."""
    import expected_floating

    a = win.shot("shadow")
    tolerance = e["popover_shadow"]["tolerance_levels"]
    middle = box[0] + box[2] // 2
    spec = e["popover_shadow"]
    worst, count = 0.0, 0
    for step in range(
        0,
        int(spec["offset_y"] * e["scale"] + 3 * spec["sigma_logical"] * e["scale"]),
        4,
    ):
        y = box[1] + box[3] + step
        if y >= a.shape[0]:
            break
        alpha = expected_floating.shadow_alpha(
            (middle + 0.5, y + 0.5), box, spec, e["scale"]
        )
        want = pixels.grey(closed, middle, y) * (1 - alpha)
        worst = max(worst, abs(pixels.grey(a, middle, y) - want))
        count += 1
    record(
        f"{screen}-shadow",
        worst <= tolerance,
        f"within {tolerance} levels at {count} samples",
        f"worst {worst:.1f} levels",
    )


def floating_screen_checks(win, e, screen, spec):
    tolerance = e["chrome"]["tab_height"]["tolerance_px"]
    if spec.get("prepare"):
        spec["prepare"](win)
        time.sleep(0.5)
    closed = win.shot("closed")
    spec["open"](win)
    shown = until(
        lambda: (win.screen(screen) or {}).get("controls"),
        f"{screen} records its controls",
        5,
    )
    box = win.screen(screen)["popover"]["box"]
    rows = sorted(
        (c for c in shown if c["kind"] in ROW_KINDS), key=lambda c: c["box"][1]
    )
    accent = round(sum(e_rgb(e, "accent")) / 3)
    card = round(sum(e_rgb(e, "card")) / 3)
    heights = []
    for row in rows[:2]:
        win.move(row["box"][0] + row["box"][2] // 2, row["box"][1] + row["box"][3] // 2)
        until(
            lambda: (
                next(
                    (c for c in win.screen(screen)["controls"] if c["id"] == row["id"]),
                    {},
                ).get("state")
                == "hover"
            ),
            "hover",
            3,
        )
        a = win.shot("row")
        top, length = run_of(
            a, row["box"][0] + 3, row["box"][1] + row["box"][3] // 2, accent, a.shape[0]
        )
        radius = e["controls"][row["kind"]]["radius"]["device"]
        length = pixels.fill_height(
            a, row["box"], accent, radius, None, HOVER_TOLERANCE
        )
        heights.append((top, length))
    want = e["floating"][spec["row"]]["device"]
    record(
        f"{screen}-row-height",
        abs(heights[0][1] - want) <= tolerance,
        f"{want:g}",
        heights[0][1],
    )
    if len(heights) > 1:
        record(
            f"{screen}-row-pitch",
            abs(heights[1][0] - heights[0][0] - want) <= tolerance,
            f"{want:g}",
            heights[1][0] - heights[0][0],
        )
    a = win.shot("open")
    border = expected_values.load_tokens()["themes"][THEME]["border"]
    over = round(sum(int(border["over"][i : i + 2], 16) for i in (1, 3, 5)) / 3)
    sep = card + border["alpha"] * (over - card)
    marks = hairline_rows(a, box, box[0] + 8, sep)
    if spec.get("input") and marks:
        height = marks[0] - box[1] + 1
        record(
            f"{screen}-input-height",
            abs(height - e["floating"][spec["input"]]["device"]) <= tolerance,
            e["floating"][spec["input"]]["device"],
            height,
        )
    if spec.get("footer") and marks:
        height = box[1] + box[3] - marks[-1]
        record(
            f"{screen}-footer-height",
            abs(height - e["floating"][spec["footer"]]["device"]) <= tolerance,
            e["floating"][spec["footer"]]["device"],
            height,
        )
    radius = popover_radius(a, box, card, e)
    want = e["floating"]["popover_radius"]["device"]
    record(
        f"{screen}-radius",
        abs(radius - want) <= tolerance,
        f"{want:g}",
        f"{radius:.2f}",
    )
    shadow_check(win, e, screen, box, closed)
    if spec.get("text"):
        palette_text_checks(win, e, screen)
    win.key("Escape")
    win.key("ctrl+u")
    time.sleep(0.3)
    if spec.get("cleanup"):
        spec["cleanup"](win)


def palette_text_checks(win, e, screen):
    """Cap heights of the palette's three texts: the typed line, a row's label and the footer's first letter. The
    typed letters HIH make the row "Go to HIH" the one match, whose capitals have flat tops."""
    import numpy as np

    win.type("HIH")
    row = until(
        lambda: next(
            (
                c
                for c in (win.screen(screen) or {}).get("controls", [])
                if c["label"] == "Go to HIH"
            ),
            None,
        ),
        "the palette lists Go to HIH",
        5,
    )
    time.sleep(0.3)
    a = win.shot("palette-text")
    card = tuple(e_rgb(e, "card"))
    inset = round(4 * e["scale"])
    roles = {
        "input": "text_title",
        "row": e["controls"]["palette-row"]["text_role"],
        "footer": "text_status",
    }

    def measure(name, box, background, first_letter=False, last_word=False):
        x, y, w, h = box
        region = (x + inset, y + 3, x + w - inset, y + h - 3)
        if last_word:
            # The label ends in the capitals HIH, 1.77 em wide in Inter; the rest of the label has round letters.
            cols = np.abs(
                a[region[1] : region[3], region[0] : region[2]] - np.array(background)
            ).max(axis=(0, 2))
            right = region[0] + int(np.where(cols >= 0.25 * cols.max())[0].max())
            em = e["cap_heights"][roles[name]]["size_px"] * e["scale"]
            region = (right - round(1.85 * em), region[1], right + 1, region[3])
        if first_letter:
            region = pixels.first_stem_box(a, region, background)
        height = None if region is None else pixels.ink_height(a, region, background)
        role = roles[name]
        text_check(
            f"palette-{name}-cap-height",
            e["cap_heights"][role]["device"],
            height,
            e["cap_heights"][role]["tolerance_px"],
        )

    measure("input", landmark(win, "popover-input"), card)
    x, y, w, h = row["box"]
    row_background = tuple(a[y + h // 2, x + w - inset])
    measure("row", row["box"], row_background, last_word=True)
    measure("footer", landmark(win, "popover-footer"), card, first_letter=True)


def find_bar_checks(win, e):
    """The pane's find bar: its field's height from the field's two edge rows (the `input` colour over `card`, in a
    column near the field's right end, clear of the placeholder and the empty count), the popover radius and shadow."""
    tolerance = e["chrome"]["tab_height"]["tolerance_px"]
    closed = win.shot("closed")
    win.key("ctrl+shift+f")  # today's window opens the find bar on Ctrl+Shift+F
    shown = until(
        lambda: (win.screen("find-bar") or {}).get("controls"),
        "find-bar records its controls",
        5,
    )
    box = win.screen("find-bar")["popover"]["box"]
    field = next(c for c in shown if c["kind"] == "field")
    time.sleep(0.3)
    a = win.shot("find-bar")
    card = round(sum(e_rgb(e, "card")) / 3)
    edge = expected_values.load_tokens()["themes"][THEME]["input"]
    over = round(sum(int(edge["over"][i : i + 2], 16) for i in (1, 3, 5)) / 3)
    column = (
        field["box"][0]
        + field["box"][2]
        - 2 * e["controls"]["field"]["radius"]["device"]
    )
    rows = hairline_rows(a, box, int(column), card + edge["alpha"] * (over - card))
    height = rows[-1] - rows[0] + 1 if len(rows) >= 2 else None
    want = e["floating"]["find_field"]["device"]
    record(
        "find-bar-field-height",
        height is not None and abs(height - want) <= tolerance,
        f"{want:g}",
        height,
    )
    radius = popover_radius(a, box, card, e)
    want = e["floating"]["popover_radius"]["device"]
    record(
        "find-bar-radius", abs(radius - want) <= tolerance, f"{want:g}", f"{radius:.2f}"
    )
    shadow_check(win, e, "find-bar", box, closed)
    win.key("Escape")
    time.sleep(0.3)


def floating_checks(win, e):
    for screen, spec in FLOATING_SCREENS.items():
        if FLOATING_ONLY and screen not in FLOATING_ONLY:
            continue
        guarded(
            f"{screen}-row-height",
            lambda s=screen, p=spec: floating_screen_checks(win, e, s, p),
        )
    guarded("find-bar-field-height", lambda: find_bar_checks(win, e))


# ---------------------------------------------------------------- G4: the Settings window

SETTINGS_PAGE = "Appearance"  # its first group label starts with a flat-topped letter, as the expected offset assumes
BORDER_LEVEL_TOLERANCE = 3  # colour levels, for the card's border pixel


def ink_left(a, box, bg):
    """The left edge of the ink in box, its first column counted by how much of the next column's ink it holds, so
    a stem that starts part-way into a pixel is placed between pixels, not on the whole one."""
    import numpy as np

    x0, y0, x1, y1 = box
    d = np.abs(a[y0 : y1 + 1, x0 : x1 + 1] - np.array(bg)).max(axis=2).astype(float)
    profile = d.sum(axis=0)
    cols = np.where(profile > 0.03 * profile.max())[0]
    c = int(cols[0])
    share = (
        min(1.0, profile[c] / max(1.0, profile[c + 1])) if c + 1 < len(profile) else 1.0
    )
    return x0 + c + 1 - share


def first_glyph(a, region, bg, text_px):
    """The box of the first glyph of a line of text: from the ink's left edge, 0.6 em wide, so a "T" is taken
    whole and the taller letters after it (a "b" is higher than a capital) are left out."""
    clusters = pixels.ink_clusters(a, region, bg)
    if not clusters:
        return None
    left = int(clusters[0][0])
    return (left, region[1], left + int(0.6 * text_px) + 1, region[3])


def glyph_extent(a, box, bg):
    """(top edge, height) of a "T" in box, in window rows. The top is the bar's first row, counted by how much of
    full ink it holds (the bar can be thinner than a pixel and spill over two rows); the bottom is the stem's last
    row, counted the same way. The two edges are read apart because the bar is wider than the stem, so a row-sum
    coverage (chrome_pixels.coverage_height) counts a spilled bar row as whole."""
    import numpy as np

    x0, y0, x1, y1 = box
    d = np.abs(a[y0:y1, x0:x1] - np.array(bg)).max(axis=2).astype(float)
    if d.max() < 12:
        return None
    cov = d / d.max()
    stem = int(np.argmax((cov >= 0.5).sum(axis=0)))
    stem_cols = slice(max(stem - 1, 0), stem + 2)
    rows = np.where(cov.max(axis=1) > 0.03)[0]
    top = y0 + int(rows.min()) + 1 - min(1.0, float(cov[rows.min()].max()))
    stem_rows = np.where(cov[:, stem_cols].max(axis=1) > 0.03)[0]
    last = int(stem_rows.max())
    bottom = y0 + last + min(1.0, float(cov[last, stem_cols].max()))
    return top, bottom - top


def card_corner_radius(a, card, inside, body, reach):
    """The radius of the first card's top-left corner: a circle fitted to the coverage of the card's edge colour
    over the page, the card's inside counted whole. The border is brighter than the fill, so an area measure would
    count it twice."""
    import numpy as np

    mid, y_mid = card[0] + card[2] // 2, card[1] + card[3] // 2
    left, _ = pixels.walk(a, mid, y_mid, -1, 0, inside)
    _, top = pixels.walk(a, mid, card[1] + 4, 0, -1, inside)
    edge = tuple(int(v) for v in a[y_mid, left])
    k = int(np.argmax([abs(p - q) for p, q in zip(edge, body)]))
    coverage = np.clip((a[..., k] - body[k]) / (edge[k] - body[k]), 0, 1)
    coverage = np.where(np.abs(a - np.array(inside)).max(axis=2) <= 2, 1.0, coverage)
    return pixels.circle_fit_radius(coverage, left, top, y_mid, mid, 3 * reach, reach)


def open_settings_appearance(win):
    """Opens Settings with Ctrl+, and shows the Appearance page by clicking its switcher segment, then moves the
    pointer away so that no hover colours the measurements."""
    win.away()
    win.key("ctrl+comma")
    until(lambda: (win.screen("settings") or {}).get("controls"), "Settings opens", 5)
    segment = next(
        (
            c
            for c in win.screen("settings")["controls"]
            if c["kind"] == "segment"
            and c["group"] == "settings-pages"
            and c["label"] == SETTINGS_PAGE
        ),
        None,
    )
    if segment is None:
        raise Fail(f"no Settings page {SETTINGS_PAGE} in the record")
    x, y, w, h = segment["box"]
    win.move(x + w // 2, y + h // 2)
    win.x("mousedown", "1")
    win.x("mouseup", "1")
    until(
        lambda: (win.screen("settings") or {}).get("page") == SETTINGS_PAGE,
        f"the {SETTINGS_PAGE} page is shown",
        3,
    )
    win.away()
    time.sleep(0.3)


def settings_checks(win, e):
    """The 12 Settings rows of the old design gate, on the Appearance page: geometry from the record's landmarks
    (the panel's edges and the divider from the pixels), fills, corners, border and text from the screenshot."""
    import numpy as np

    open_settings_appearance(win)
    try:
        a = win.shot("settings")
        settings = e["settings"]
        tol = settings["header_height"]["tolerance_px"]
        header = landmark(win, "settings-header")
        pill = landmark(win, "settings-tab-pill")
        card = landmark(win, "settings-card", 0)
        card_rgb = e_rgb(e, "card")

        # The header: from the panel's top border down to the divider, in a column left of the pill.
        probe = pill[0] - 6
        header_row = header[1] + header[3] // 2
        header_colour = tuple(int(v) for v in a[header_row, probe])
        _, top = pixels.walk(a, probe, header_row, 0, -1, header_colour)
        _, divider = pixels.walk(a, probe, header_row, 0, 1, header_colour)
        height = divider - top + 1
        want = settings["header_height"]["device"]
        record(
            "settings-header-height",
            abs(height - want) <= tol and abs(header[3] - want) <= tol,
            f"{want:g}",
            f"{height} (landmark {header[3]})",
        )

        # The pill: the height of its fill in the tray colour.
        want = settings["tab_pill_height"]["device"]
        pill_height = pixels.fill_height(
            a,
            pill,
            sum(card_rgb) / 3,
            e["controls"]["button"]["radius"]["device"],
        )
        record(
            "settings-tab-pill-height",
            abs(pill_height - want) <= tol,
            f"{want:g}",
            pill_height,
        )

        history = next(
            c
            for c in win.screen("settings")["controls"]
            if c["group"] == "settings-pages" and c["label"] == "History"
        )
        hx, hy, hw, hh = history["box"]
        tray = tuple(int(v) for v in a[hy + hh // 2, hx + 1])
        stem = pixels.first_stem_box(
            a, (hx + 2, hy + 2, hx + hw - 2, hy + hh - 2), tray
        )
        role = e["cap_heights"]["text_tab_switch"]
        text_check(
            "settings-tab-cap-height",
            role["device"],
            None if stem is None else pixels.ink_height(a, stem, tray),
            role["tolerance_px"],
        )

        # The first card's sides, along a row just below its corners' arcs.
        radius = settings["card_radius"]["device"]
        side_row = card[1] + int(np.ceil(radius)) + 1
        inside = tuple(int(v) for v in a[side_row, card[0] + 4])
        card_left, _ = pixels.walk(a, card[0] + card[2] // 2, side_row, -1, 0, inside)
        card_right, _ = pixels.walk(a, card[0] + card[2] // 2, side_row, 1, 0, inside)
        width = card_right - card_left + 1
        want = settings["column_max"]["device"]
        record(
            "settings-column-width",
            width <= want + settings["column_max"]["tolerance_px"],
            f"<={want:g}",
            width,
        )
        body = tuple(int(v) for v in a[side_row, card_left - 3])
        panel_left, _ = pixels.walk(a, card_left - 3, side_row, -1, 0, body)
        panel_right, _ = pixels.walk(a, card_right + 3, side_row, 1, 0, body)
        sides = (card_left - panel_left, panel_right - card_right)
        want = settings["padding_side"]["device"]
        record(
            "settings-side-padding",
            all(abs(v - want) <= tol for v in sides),
            f"{want:g}",
            f"{sides[0]} left, {sides[1]} right",
        )

        lx, ly, lw, lh = landmark(win, "settings-group-label", 0)
        label = pixels.first_stem_box(a, (lx, ly, lx + 60, ly + lh - 1), body)
        span = None if label is None else pixels.ink_span(a, label, body)
        header_bottom = header[1] + header[3]
        want = e["settings_offsets"]["top_padding"]["device"]
        record(
            "settings-top-padding",
            span is not None and abs(span[0] - header_bottom - want) <= tol,
            f"{want:.2f}",
            "no ink" if span is None else f"{span[0] - header_bottom:.2f}",
        )

        radius_observed = card_corner_radius(
            a, card, inside, body, int(np.ceil(radius))
        )
        record(
            "settings-card-radius",
            abs(radius_observed - radius) <= tol,
            f"{radius:g}",
            f"{radius_observed:.2f}",
        )

        y_mid = card[1] + card[3] // 2
        edge = tuple(int(v) for v in a[y_mid, card[0]])
        interior = tuple(int(v) for v in a[y_mid, card[0] + 3])
        share = e["settings_card_border_share"]
        over = e["settings_card_border_over"]
        want_edge = tuple(f + share * (o - f) for f, o in zip(interior, over))
        record(
            "settings-card-border-alpha",
            all(abs(p - q) <= BORDER_LEVEL_TOLERANCE for p, q in zip(edge, want_edge)),
            f"{share:.3f} (rgb {tuple(round(v) for v in want_edge)})",
            f"rgb {edge} over {interior}",
        )

        # The title and the description, from their landmarks: the title's ink left and cap top, both cap heights.
        title = landmark(win, "settings-card-title", 0)
        description = landmark(win, "settings-card-description", 0)

        def region(box):
            x, y, w, h = box
            return (card[0] + 3, y - 1, min(x + w, card[0] + card[2] // 2), y + h)

        title_region = region(title)
        text_px = e["cap_heights"]["text_title"]["size_px"] * e["scale"]
        title_glyph = first_glyph(a, title_region, interior, text_px)
        title_extent = (
            None if title_glyph is None else glyph_extent(a, title_glyph, interior)
        )
        top = None if title_extent is None else title_extent[0]
        padding_x = ink_left(a, title_region, interior) - card[0]
        want = e["settings_offsets"]["card_padding_x"]["device"]
        record(
            "settings-card-padding-x",
            abs(padding_x - want) <= tol,
            f"{want:.2f}",
            f"{padding_x:.2f}",
        )
        want = e["settings_offsets"]["card_padding_y"]["device"]
        record(
            "settings-card-padding-y",
            top is not None and abs(top - card[1] - want) <= tol,
            f"{want:.2f}",
            "no ink" if top is None else f"{top - card[1]:.2f}",
        )
        description_glyph = first_glyph(
            a,
            region(description),
            interior,
            e["cap_heights"]["text_desc"]["size_px"] * e["scale"],
        )
        description_extent = (
            None
            if description_glyph is None
            else glyph_extent(a, description_glyph, interior)
        )
        for check, extent, role in (
            ("settings-title-cap-height", title_extent, "text_title"),
            ("settings-description-cap-height", description_extent, "text_desc"),
        ):
            text_check(
                check,
                e["cap_heights"][role]["device"],
                None if extent is None else extent[1],
                e["cap_heights"][role]["tolerance_px"],
            )
    finally:
        win.key("Escape")
        time.sleep(0.3)


def make_window(scale):
    if SIMULATE and SETTINGS:
        from simulated_settings import SimSettingsWindow

        return SimSettingsWindow(scale, THEME, FAULT)
    if SIMULATE and FLOATING:
        from simulated_floating import SimFloatingWindow

        return SimFloatingWindow(scale, THEME, FAULT)
    if SIMULATE:
        return SimWindow(scale, THEME, FAULT if FAULT in ("tab-too-tall", "pane-rounded") else None)
    return RealWindow(scale, WORK, APP, CONFIG, FAULT, KEEP)


def main():
    if FAULT not in (None, *FAULTS):
        print(f"FAIL: unknown fault {FAULT}")
        sys.exit(1)
    check_sources()
    if FAULT == "loose-literal":
        return
    if not SIMULATE:
        for tool in ("Xvfb", "xdotool", "ffmpeg"):
            if not shutil.which(tool):
                record("setup", False, f"{tool} installed", "missing")
                return
        if WORK.exists():
            shutil.rmtree(WORK)
        WORK.mkdir(mode=0o700, parents=True)
    try:
        for scale in SCALES:
            builder = (
                expected_settings.build
                if SETTINGS
                else expected_floating.build
                if FLOATING
                else expected_values.build
            )
            e = builder(scale, THEME)
            win = make_window(scale)
            try:
                window_checks(win, e)
            except Fail as problem:
                record("window", False, "a window that records", str(problem)[:200])
            finally:
                win.close()
    finally:
        if not SIMULATE and not KEEP:
            shutil.rmtree(WORK, ignore_errors=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # a crash still reports FAIL, never PASS
        record("gate", False, "no crash", f"{type(e).__name__}: {e}")
    if all(ok for _, ok, _, _ in RESULTS) and RESULTS:
        print("PASS: design tokens")
        sys.exit(0)
    sys.exit(1)
