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
import chrome_pixels as pixels  # noqa: E402
import expected as expected_values  # noqa: E402
from chrome_harness import Fail, RealWindow  # noqa: E402
from simulated_window import SimWindow  # noqa: E402

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


FAULT = arg("--fault")
FAULTS = {"tab-too-tall": "tab-height", "loose-literal": "loose-numbers"}
SIMULATE = "--simulate" in ARGS
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
    startup_animation=False,
    history=False,
    animations=False,
    cursor_blink="off",
    theme="dark",
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


def pane_gap_margin(e, a, pane, frame):
    """The pane gap and the window margin, from the pixels along the pane's middle row."""
    y_mid = pane[1] + pane[3] // 2
    left = pane[0]
    while left > 0 and pixels.cdist(a[y_mid, left - 1], frame) <= 2:
        left -= 1
    margin = pane[0] - left
    x = pane[0] + pane[2] // 2
    while pixels.cdist(a[y_mid, x], frame) > 2 and x < a.shape[1] - 1:
        x += 1
    gap_start = x
    while pixels.cdist(a[y_mid, x], frame) <= 2 and x < a.shape[1] - 1:
        x += 1
    return margin, x - gap_start


def window_checks(win, e):
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
        margin, between = pane_gap_margin(e, a, pane, frame)
        record(
            "pane-margin",
            abs(margin - chrome["pane_margin"]["device"]) <= tol,
            chrome["pane_margin"]["device"],
            margin,
        )
        record(
            "pane-gap",
            abs(between - chrome["pane_gap"]["device"]) <= tol,
            chrome["pane_gap"]["device"],
            between,
        )
        y_mid = pane[1] + pane[3] // 2
        left = pane[0]
        edge = tuple(int(v) for v in a[y_mid, left])
        want = tuple(
            int(e["colours"]["border_over_background"]["hex"][i : i + 2], 16)
            for i in (1, 3, 5)
        )
        tolerance = chrome["pane_border_alpha"]["tolerance_levels"]
        record(
            "pane-border-alpha",
            all(abs(p - q) <= tolerance for p, q in zip(edge, want)),
            f"{chrome['pane_border_alpha']['value']:.3f} (rgb {want})",
            f"rgb {edge}",
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


def e_rgb(e, role):
    return tuple(int(e["colours"][role]["hex"][i : i + 2], 16) for i in (1, 3, 5))


def e_grey(e, role):
    return round(sum(e_rgb(e, role)) / 3)


def pane_radius(a, pane, frame, e):
    """A circle fitted to the pane's top-left corner. The border sits inside the pane's box over the fill, so the fill
    is fitted (the border row and column count as outside it) and the border's width is added back."""
    import numpy as np

    x, y, w, h = pane
    y_mid = y + h // 2
    fill = tuple(int(v) for v in a[y_mid, x + 20])
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


def make_window(scale):
    if SIMULATE:
        return SimWindow(scale, THEME, FAULT if FAULT in ("tab-too-tall",) else None)
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
            e = expected_values.build(scale, THEME)
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
