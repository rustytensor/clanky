#!/usr/bin/env python3
"""Side tabs gate: clanky-gpui's tab_style "vertical" side column, its rows, groups, states and limits.

Usage: gate_side_tabs.py --app PATH [--fault side-width] [--only N,N...] [--keep]
Prints one line per gate, "PASS <n> <name>" or "FAIL <n> <name>: <why>", then "PASS" (exit 0) or "FAIL" (exit 1).
scripts/gate-s1-side-tabs.sh builds the window and runs this file.

Every expected value below was fixed before the window code was written; none is read from the program under test.
The window runs at scale 1 (device px = logical px), 1200 x 800, the Graphite dark theme, on a private Xvfb with an
isolated HOME (side_window.py, on the chrome harness). The record is RECORD.md's control record plus these landmark
lines: side-tabs (the column's outer box, [0,0,0,0] with tab_style horizontal), side-tab-row (one per drawn row:
tab, text, sub, mark, agent, state, state_label, unread, group), side-tab-group (a group header: text),
tools-panel ([0,0,0,0] when closed) and tab-dot.

The vertical window's tabs, each typed into through the owner API:
  0 ~/alpha (repo, branch side-gate), then `true`        group alpha, sub side-gate, state "", mark shell
  1 ~/alpha/src, then `cat` waiting for input            group alpha, waiting / needs you
  2 ~/beta (repo, branch main), then `false`             group beta, failed / exit 1, badge err
  3 ~/plain-folder (no repo), then `true`                group Other, sub ~/plain-folder
  4 ~/beta, then `pi` (the fake agent, reading stdin)    group beta, mark pi's "π", agent, waiting, badge agent
  5 ~/beta, then `pi </dev/null` (an agent, exit 0)      group beta, done, badge ok
  6 the untrusted folder (ESC, BEL, U+202E in its name)  group Other, text escaped to U+FFFD

Gates and their expected values:
   1 standards       cargo fmt -p clanky-gpui -- --check and cargo clippy -j 8 -p clanky-gpui --all-targets --no-deps -- -D warnings exit 0;
                     the design gate's loose-numbers check finds 0 hits in crates/clanky-gpui/src.
   2 horizontal      default tab_style, 2 tabs: side-tabs [0,0,0,0]; 2 "tab" controls; gate-controls.py passes.
   3 vertical        3 tabs: side-tabs width 236 +-0.5; 3 list-row controls (group side-tabs), each 48 tall; no
                     "tab" control.
   4 states          tab 1 waiting / needs you; tab 2 failed / exit 1; tab 0 "" / "".
   5 agents          tab 4 mark "π", agent true, state waiting; tab 0 mark "shell", agent false.
   6 sub             tab 0 sub "side-gate"; tab 3 sub "~/plain-folder".
   7 groups          group headers in index order ["alpha", "beta", "Other"]; rows in the order of tabs
                     [0, 1, 2, 4, 5, 3, 6], their group alpha, alpha, beta, Other, beta, beta, Other; with tab_groups
                     none: no side-tab-group landmark, rows in tab order, every row's group null.
   8 select-unread   a click on row 7001 makes the workspace's active index 1 and that row checked; output into
                     inactive tab 3 sets its unread; a click on its row clears it and makes it active.
   9 fold            the title bar's id 2 is "Side tabs", checked; a click folds the column to 52 +-0.5, unchecked,
                     every row's label_box null; a second click unfolds it to 236 +-0.5.
  10 filter          typing "beta" in field 6990 leaves exactly the rows whose title, sub or group holds "beta"
                     (tabs 2, 4, 5) and field_text "beta"; Escape brings every row back and field_text "".
  11 tools-panel     Ctrl+Shift+B in vertical mode: the tools-panel box's right edge is 1200 +-0.5; side-tabs x 0.
  12 pixels          the active row (tab 0) at 3 px from its left edge, mid-height: the foreground at 7 % over the
                     frame colour, +-2 per channel; the badges of tabs 4, 5 and 2 at their centres #dfae74, #7cc48f,
                     #e97e7e, +-8 per channel (a badge is the largest patch of its colour left of the row's text).
  13 bad-config      tab_style vertical, tab_groups "maybe": the window starts; its stderr holds
                     tab_groups must be "project" or "none"; groups behave as project (with the column shown, the
                     one group of a tab in ~/HIH is "Other"; a window that fell back to every default draws no
                     column and writes no group header).
  14 untrusted-text  tab 6's row: text is the folder name with ESC, BEL and U+202E each replaced by U+FFFD (3 of
                     them) and no character below U+0020 in text, sub or the control's label; the row is 48 tall.
  15 limits          60 tabs: the column and the first pane stay inside the 1200 x 800 window, the pane right of
                     the column; 5 wheel notches down over the column move a row up by at least one row (48);
                     CLANKY_REQUEST_LOG over 5 s: lines written, and at most panes x 6 ticks terminal_info and as
                     many input_state lines.
  16 idle            vertical, animations on, no working or waiting row: 0 new frames in 5 s; tab 1 running
                     `sleep` (working): at least 10 frames in 5 s while focused (the spinner's 4 steps a second),
                     then 0 new frames in 5 s once the window has lost the focus. Frames are frames.csv lines and
                     record "frame" lines.
  17 whole-project   scripts/gate-c5-panes.sh (CLANKY_BIN set), scripts/gate-floating.sh and
                     scripts/window-chrome/gate-controls.py each exit 0 with their PASS line.
  18 planted-fault   CLANKY_GATE_FAULT=side-width scripts/gate-s1-side-tabs.sh prints FAIL, with gate 3 failing on
                     "side-tabs width: expected 236 +-0.5, observed 240".

With --fault side-width the run plants the window's side-width fault (CLANKY_CONTROL_FAULT) and runs gate 3 only.
"""

import contextlib
import importlib.util
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections import deque
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "window-chrome"))
import expected as chrome_expected
from chrome_harness import Fail
from side_window import UNTRUSTED_NAME, SideWindow

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


APP = Path(
    arg("--app", os.environ.get("CLANKY_GPUI_BIN", REPO / "target/release/clanky-gpui"))
)
FAULT = arg("--fault")
KEEP = "--keep" in ARGS
ONLY = {int(n) for n in arg("--only", "").split(",") if n}
if FAULT == "side-width":
    ONLY = {3}
NAMES = {
    1: "standards",
    2: "horizontal",
    3: "vertical",
    4: "states",
    5: "agents",
    6: "sub",
    7: "groups",
    8: "select-unread",
    9: "fold",
    10: "filter",
    11: "tools-panel",
    12: "pixels",
    13: "bad-config",
    14: "untrusted-text",
    15: "limits",
    16: "idle",
    17: "whole-project",
    18: "planted-fault",
}

WINDOW_W, WINDOW_H = 1200, 800
SIDE_WIDTH, FOLDED_WIDTH, ROW_HEIGHT, WIDTH_TOL = 236, 52, 48, 0.5
FILTER_ID, NEW_TAB_ID, FOLD_ID, FIRST_ROW_ID = 6990, 1, 2, 7000
BADGE = {
    "agent": (0xDF, 0xAE, 0x74),
    "ok": (0x7C, 0xC4, 0x8F),
    "err": (0xE9, 0x7E, 0x7E),
}
BADGE_TOL, FILL_TOL, BADGE_MIN_PIXELS = 8, 2, 20
ACTIVE_FILL_ALPHA = 0.07
BAD_CONFIG_MESSAGE = 'tab_groups must be "project" or "none"'
IDLE_SECONDS, TICKS_IN_IDLE = 5, 6
SPINNER_MIN_FRAMES = 10
LIMIT_TABS = 60
# Characters that must never reach the screen or the record raw (C0, DEL and C1, the bidi embeddings and isolates).
UNSAFE = [(0x00, 0x1F), (0x7F, 0x9F), (0x202A, 0x202E), (0x2066, 0x2069)]

BASE_CONFIG = {
    "restore_layout": False,
    "new_window_panes": 1,
    "startup_animation": False,
    "history": False,
    "animations": False,
    "cursor_blink": "off",
    "theme": "dark",
    "tools_panel": "off",
}
STATES_WAIT = 25


def agents_config():
    return [{"name": "pi", "command": "pi"}]


def escaped(text):
    return "".join(
        "�" if any(lo <= ord(ch) <= hi for lo, hi in UNSAFE) else ch for ch in text
    )


def has_raw_unsafe(text):
    return any(any(lo <= ord(ch) <= hi for lo, hi in UNSAFE) for ch in text or "")


def near(value, want, tol):
    return value is not None and abs(value - want) <= tol


class Results:
    """Each gate's parts: a gate passes when it has parts and every part passed."""

    def __init__(self):
        self.parts = {n: [] for n in NAMES}

    def check(self, n, ok, why):
        self.parts[n].append((bool(ok), why))
        return ok

    def crash(self, gates, problem):
        for n in gates:
            self.parts[n].append((False, problem))

    def lines(self, wanted):
        out = []
        for n, name in NAMES.items():
            if n not in wanted:
                continue
            parts = self.parts[n]
            failed = [why for ok, why in parts if not ok]
            if parts and not failed:
                out.append(f"PASS {n} {name}")
            else:
                out.append(f"FAIL {n} {name}: {'; '.join(failed) or 'not run'}")
        return out


R = Results()


def progress(text):
    print(f"... {text}", file=sys.stderr, flush=True)


def wants(*gates):
    return not ONLY or any(n in ONLY for n in gates)


def window(work, name, config, **kw):
    progress(f"window {name}")
    return SideWindow(work / name, APP, config, fault=FAULT, keep=KEEP, **kw)


def soft_until(test, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if test():
            return True
        time.sleep(0.1)
    return bool(test())


def side_box(win):
    return win.landmark_box("side-tabs")


def width_of(box):
    return box[2] if box else None


def theme_values():
    tokens = chrome_expected.load_tokens()
    colours = chrome_expected.theme_colours(tokens["themes"]["graphite-dark"])
    frame = chrome_expected.hex_to_rgb(colours["frame"]["hex"])
    foreground = chrome_expected.hex_to_rgb(colours["foreground"]["hex"])
    return frame, foreground


# ---------------------------------------------------------------- gate 1: standards


def standards():
    for argv in (
        # The window crate is rustfmt-clean; the older crates keep a compact hand style, so only this one is checked.
        ["cargo", "fmt", "-p", "clanky-gpui", "--", "--check"],
        # --no-deps: the path dependencies carry findings from newer clippy lints that predate this crate's code.
        [
            "cargo",
            "clippy",
            "-q",
            "-j",
            "8",
            "-p",
            "clanky-gpui",
            "--all-targets",
            "--no-deps",
            "--",
            "-D",
            "warnings",
        ],
    ):
        progress(" ".join(argv))
        done = subprocess.run(
            argv, check=False, cwd=REPO, capture_output=True, text=True
        )
        # rustfmt's notes on nightly-only options are not the reason; the first lines of the diff or error are.
        lines = [
            line
            for line in (done.stdout + done.stderr).splitlines()
            if line.strip() and not line.startswith("Warning: can't set")
        ]
        tail = lines[:3]
        R.check(
            1,
            done.returncode == 0,
            f"{' '.join(argv[:2])} exits 0 (exit {done.returncode}: {' | '.join(tail)[:300]})",
        )
    # The design gate's check 1, run from its own module on the window crate's sources.
    saved = sys.argv
    sys.argv = [saved[0]]
    try:
        spec = importlib.util.spec_from_file_location(
            "design_tokens", REPO / "scripts/window-chrome/gate-design-tokens.py"
        )
        design = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(design)
        # The design gate prints its own result line; this gate's output is one line per gate.
        with contextlib.redirect_stdout(sys.stderr):
            design.check_sources()
        found = [r for r in design.RESULTS if r[0] == "loose-numbers"]
        R.check(
            1,
            found and found[0][1],
            f"loose numbers: expected 0, observed {found[0][3] if found else 'no result'}",
        )
    finally:
        sys.argv = saved


# ---------------------------------------------------------------- gate 2 and 17: today's gates


_controls_gate = None


def controls_gate(work):
    """scripts/window-chrome/gate-controls.py on the binary, as gate-window-chrome.sh runs it; run once."""
    global _controls_gate
    if _controls_gate is None:
        progress("gate-controls.py")
        env = dict(os.environ, CLANKY_CONTROLS_WORK=str(work / "controls-gate"))
        done = subprocess.run(  # noqa: PLW1510 - the exit status is the result
            [
                sys.executable,
                str(REPO / "scripts/window-chrome/gate-controls.py"),
                "--app",
                str(APP),
            ],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
        )
        out = done.stdout + done.stderr
        ok = done.returncode == 0 and "PASS: controls" in out
        fails = [line for line in out.splitlines() if line.startswith("FAIL")][:3]
        _controls_gate = (
            ok,
            f"gate-controls.py passes (exit {done.returncode}: {' | '.join(fails)[:300]})",
        )
    return _controls_gate


def horizontal(work):
    win = window(work, "horizontal", dict(BASE_CONFIG, agents=agents_config()))
    try:
        win.new_tab()
        time.sleep(2)
        box = side_box(win)
        R.check(
            2, box == [0, 0, 0, 0], f"side-tabs landmark [0,0,0,0] (observed {box})"
        )
        groups = len(win.workspace()["groups"])
        tabs = [c for c in win.controls() if c.get("kind") == "tab"]
        R.check(
            2,
            groups == 2 and len(tabs) == groups,
            f"{groups} tab controls for {groups} tabs (observed {len(tabs)})",
        )
    finally:
        win.close()
    R.check(2, *controls_gate(work))


def whole_project(work):
    env = {
        k: v
        for k, v in os.environ.items()
        if k not in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS")
    }
    for label, argv, extra, want in (
        (
            "gate-c5-panes.sh",
            ["bash", "scripts/gate-c5-panes.sh"],
            {"CLANKY_BIN": str(APP), "CLANKY_C5_WORK": str(work / "c5-gate")},
            "PASS: C5",
        ),
        (
            "gate-floating.sh",
            ["bash", "scripts/gate-floating.sh"],
            {"CLANKY_GPUI_BIN": str(APP)},
            "PASS: floating screens",
        ),
    ):
        progress(label)
        done = subprocess.run(
            argv,
            check=False,
            cwd=REPO,
            env=dict(env, **extra),
            capture_output=True,
            text=True,
        )
        out = (done.stdout + done.stderr).strip()
        last = out.splitlines()[-1] if out else ""
        fails = [line for line in out.splitlines() if line.startswith("FAIL")][:3]
        R.check(
            17,
            done.returncode == 0 and last.startswith(want),
            f"{label} passes (exit {done.returncode}: {' | '.join(fails or [last])[:300]})",
        )
    R.check(17, *controls_gate(work))


# ---------------------------------------------------------------- the vertical window: gates 3 to 12 and 14


def setup_tab(win, tab, *commands):
    for command in commands:
        win.send(tab, command + "\r")
        time.sleep(0.4)


def vertical(work):
    config = dict(BASE_CONFIG, tab_style="vertical", agents=agents_config())
    win = window(work, "vertical", config)
    try:
        setup_tab(win, 0, "cd ~/alpha", "true")
        setup_tab(win, win.new_tab(), "cd ~/alpha/src", "cat")
        setup_tab(win, win.new_tab(), "cd ~/beta", "false")
        gate_three_tabs(win)
        if ONLY == {3}:
            return
        setup_tab(win, win.new_tab(), "cd ~/plain-folder", "true")
        setup_tab(win, win.new_tab(), "cd ~/beta", "pi")
        setup_tab(win, win.new_tab(), "cd ~/beta", "pi </dev/null")
        setup_tab(win, win.new_tab(), 'cd "$(cat ~/weird-path.txt)"')

        def settled():
            rows = win.rows_by_tab()
            return (
                len(rows) >= 7
                and rows.get(1, {}).get("state") == "waiting"
                and rows.get(2, {}).get("state") == "failed"
                and rows.get(4, {}).get("state") == "waiting"
                and rows.get(5, {}).get("state") == "done"
                and rows.get(6, {}).get("text") == escaped(UNTRUSTED_NAME)
            )

        soft_until(settled, STATES_WAIT)
        rows = win.rows_by_tab()
        data_gates(win, rows)
        if wants(12):
            pixel_gate(win)
        if wants(8):
            select_gate(win)
        if wants(10):
            filter_gate(win, rows)
        if wants(9):
            fold_gate(win)
        if wants(11):
            tools_gate(win)
    finally:
        win.close()


def gate_three_tabs(win):
    def ready():
        return len(win.rows()) == 3 and side_box(win)

    soft_until(ready, 8)
    box = side_box(win)
    R.check(3, box is not None, "the record has a side-tabs landmark")
    R.check(
        3,
        near(width_of(box), SIDE_WIDTH, WIDTH_TOL),
        f"side-tabs width: expected {SIDE_WIDTH} +-{WIDTH_TOL}, observed {width_of(box)}",
    )
    rows = win.rows()
    R.check(
        3,
        len(rows) == 3,
        f"3 list-row controls in group side-tabs (observed {len(rows)})",
    )
    heights = [r["box"][3] for r in rows]
    R.check(
        3,
        rows and all(h == ROW_HEIGHT for h in heights),
        f"every row {ROW_HEIGHT} tall (observed {heights})",
    )
    tabs = [c for c in win.controls() if c.get("kind") == "tab"]
    R.check(3, not tabs, f"no tab controls in the title bar (observed {len(tabs)})")


def field(rows, tab, name):
    return rows.get(tab, {}).get(name, "<no row>")


def data_gates(win, rows):
    for tab, state, label in (
        (1, "waiting", "needs you"),
        (2, "failed", "exit 1"),
        (0, "", ""),
    ):
        got = (field(rows, tab, "state"), field(rows, tab, "state_label"))
        R.check(
            4,
            got == (state, label),
            f"tab {tab} {state!r}/{label!r} (observed {got[0]!r}/{got[1]!r})",
        )
    got = (field(rows, 4, "mark"), field(rows, 4, "agent"), field(rows, 4, "state"))
    R.check(
        5,
        got == ("π", True, "waiting"),
        f"tab 4 (pi) mark π, agent, waiting (observed {got})",
    )
    got = (field(rows, 0, "mark"), field(rows, 0, "agent"))
    R.check(
        5, got == ("shell", False), f"tab 0 mark shell, not an agent (observed {got})"
    )
    for tab, sub in ((0, "side-gate"), (3, "~/plain-folder")):
        R.check(
            6,
            field(rows, tab, "sub") == sub,
            f"tab {tab} sub {sub!r} (observed {field(rows, tab, 'sub')!r})",
        )
    groups = win.landmark_lines("side-tab-group")
    texts = [groups[i].get("text") for i in sorted(groups)]
    R.check(
        7,
        texts == ["alpha", "beta", "Other"],
        f"group headers ['alpha', 'beta', 'Other'] (observed {texts})",
    )
    order = [
        line.get("tab")
        for line in sorted(rows.values(), key=lambda line: line.get("index", 0))
    ]
    R.check(
        7,
        order == [0, 1, 2, 4, 5, 3, 6],
        f"rows in the order of tabs [0, 1, 2, 4, 5, 3, 6] (observed {order})",
    )
    want = {
        0: "alpha",
        1: "alpha",
        2: "beta",
        3: "Other",
        4: "beta",
        5: "beta",
        6: "Other",
    }
    got = {tab: field(rows, tab, "group") for tab in want}
    R.check(7, got == want, f"row groups {want} (observed {got})")
    row = rows.get(6, {})
    text, sub = row.get("text"), row.get("sub")
    R.check(
        14,
        text == escaped(UNTRUSTED_NAME),
        f"tab 6 text is the escaped folder name (observed {text!r:.80})",
    )
    R.check(
        14,
        isinstance(text, str) and text.count("�") == 3,
        f"3 U+FFFD in the text (observed {text!r:.80})",
    )
    control = win.control(FIRST_ROW_ID + 6)
    label = control.get("label") if control else None
    for what, value in (("text", text), ("sub", sub), ("control label", label)):
        R.check(
            14,
            isinstance(value, str) and not has_raw_unsafe(value),
            f"no unsafe character in the {what}",
        )
    R.check(
        14,
        row.get("box", [0, 0, 0, 0])[3] == ROW_HEIGHT,
        f"tab 6 row {ROW_HEIGHT} tall (observed {row.get('box')})",
    )
    R.check(
        14,
        control is not None and control["box"][3] == ROW_HEIGHT,
        f"tab 6 control {ROW_HEIGHT} tall (observed {control and control['box']})",
    )


def badge_centre(image, box, colour):
    """The centre of the largest patch of `colour` (+-BADGE_TOL) in the icon area left of the row's text."""
    x, y, w, h = box
    area = image[y : y + h, x : x + min(56, w)]
    mask = np.abs(area - np.array(colour)).max(axis=2) <= BADGE_TOL
    seen = np.zeros_like(mask)
    best = []
    for sy, sx in zip(*np.nonzero(mask)):
        if seen[sy, sx]:
            continue
        patch, queue = [], deque([(sy, sx)])
        seen[sy, sx] = True
        while queue:
            py, px = queue.popleft()
            patch.append((py, px))
            for ny, nx in ((py + 1, px), (py - 1, px), (py, px + 1), (py, px - 1)):
                if (
                    0 <= ny < mask.shape[0]
                    and 0 <= nx < mask.shape[1]
                    and mask[ny, nx]
                    and not seen[ny, nx]
                ):
                    seen[ny, nx] = True
                    queue.append((ny, nx))
        if len(patch) > len(best):
            best = patch
    if len(best) < BADGE_MIN_PIXELS:
        return None, len(best)
    ys, xs = [p[0] for p in best], [p[1] for p in best]
    return (x + (min(xs) + max(xs)) // 2, y + (min(ys) + max(ys)) // 2), len(best)


def pixel_gate(win):
    # Every tab once with Ctrl+Tab, so no row is unread (an unread badge holds a dot). A full cycle ends on the tab
    # that was active before it (the last one opened), so tab 0 is then chosen by a click on its row.
    count = len(win.workspace()["groups"])
    win.x("windowfocus", "--sync", win.wid)
    for _ in range(count):
        win.key("ctrl+Tab")
        time.sleep(0.3)
    try:
        click_row(win, 0)
    except Fail as problem:
        R.check(12, False, str(problem))
    soft_until(lambda: win.workspace()["active"] == 0, 3)
    win.away()
    time.sleep(1.5)
    frame, foreground = theme_values()
    want = tuple(
        round(v) for v in chrome_expected.blend(frame, foreground, ACTIVE_FILL_ALPHA)
    )
    image = win.shot("pixels")
    row = win.control(FIRST_ROW_ID)
    R.check(
        12,
        row is not None and row.get("checked") is True,
        "tab 0's row is the checked one",
    )
    if row:
        x, y, _, h = row["box"]
        got = tuple(int(v) for v in image[y + h // 2, x + 3])
        R.check(
            12,
            all(abs(a - b) <= FILL_TOL for a, b in zip(got, want)),
            f"active row fill {want} +-{FILL_TOL} (observed {got})",
        )
    for tab, role in ((4, "agent"), (5, "ok"), (2, "err")):
        control = win.control(FIRST_ROW_ID + tab)
        if not control:
            R.check(12, False, f"tab {tab}'s row control for its {role} badge")
            continue
        centre, size = badge_centre(image, control["box"], BADGE[role])
        got = tuple(int(v) for v in image[centre[1], centre[0]]) if centre else None
        R.check(
            12,
            got is not None
            and all(abs(a - b) <= BADGE_TOL for a, b in zip(got, BADGE[role])),
            f"tab {tab} badge centre {BADGE[role]} +-{BADGE_TOL} (observed {got}, patch {size} px)",
        )


def click_row(win, tab):
    control = win.control(FIRST_ROW_ID + tab)
    if not control:
        raise Fail(f"no row control {FIRST_ROW_ID + tab} to click")
    win.click_box(control["box"])


def select_gate(win):
    try:
        click_row(win, 1)
        soft_until(lambda: win.workspace()["active"] == 1, 4)
        active = win.workspace()["active"]
        R.check(
            8,
            active == 1,
            f"a click on row 7001 makes tab 1 active (observed active {active})",
        )
        control = win.control(FIRST_ROW_ID + 1)
        R.check(
            8,
            control is not None and control.get("checked") is True,
            "row 7001 checked",
        )
        click_row(win, 3)
        soft_until(lambda: win.workspace()["active"] == 3, 4)
        click_row(win, 0)
        soft_until(lambda: win.workspace()["active"] == 0, 4)
        win.send(3, "echo unread-probe\r")
        soft_until(lambda: win.rows_by_tab().get(3, {}).get("unread") is True, 6)
        got = win.rows_by_tab().get(3, {}).get("unread")
        R.check(
            8,
            got is True,
            f"output into inactive tab 3 makes it unread (observed {got})",
        )
        click_row(win, 3)
        soft_until(lambda: win.rows_by_tab().get(3, {}).get("unread") is False, 6)
        got = win.rows_by_tab().get(3, {}).get("unread")
        R.check(
            8,
            got is False and win.workspace()["active"] == 3,
            f"selecting tab 3 clears unread (observed {got})",
        )
    except Fail as problem:
        R.check(8, False, str(problem))


def row_tabs(win):
    return sorted(c["id"] - FIRST_ROW_ID for c in win.rows())


def filter_gate(win, rows):
    every = sorted(rows)
    want = sorted(
        tab
        for tab, line in rows.items()
        if any(
            "beta" in str(line.get(k) or "").lower() for k in ("text", "sub", "group")
        )
    )
    R.check(
        10,
        want == [2, 4, 5],
        f"the rows holding beta are tabs [2, 4, 5] (observed {want})",
    )
    control = win.control(FILTER_ID)
    if not control:
        R.check(10, False, f"no filter field {FILTER_ID}")
        return
    R.check(
        10,
        control.get("kind") == "field" and control.get("label") == "Filter tabs",
        f'field {FILTER_ID} is the field "Filter tabs" (observed {control.get("kind")} {control.get("label")!r})',
    )
    win.click_box(control["box"])
    win.type("beta")
    soft_until(lambda: row_tabs(win) == [2, 4, 5], 4)
    R.check(
        10,
        row_tabs(win) == [2, 4, 5],
        f"typing beta leaves rows [2, 4, 5] (observed {row_tabs(win)})",
    )
    text = (win.control(FILTER_ID) or {}).get("field_text")
    R.check(10, text == "beta", f"field_text 'beta' (observed {text!r})")
    win.key("Escape")
    soft_until(lambda: row_tabs(win) == every, 4)
    R.check(
        10,
        row_tabs(win) == every,
        f"Escape brings back rows {every} (observed {row_tabs(win)})",
    )
    text = (win.control(FILTER_ID) or {}).get("field_text")
    R.check(10, text == "", f"field_text '' after Escape (observed {text!r})")


def fold_gate(win):
    control = win.control(FOLD_ID)
    if not control:
        R.check(9, False, f"no control {FOLD_ID} in the title bar")
        return
    R.check(
        9,
        control.get("label") == "Side tabs" and control.get("checked") is True,
        f'control 2 is "Side tabs", checked (observed {control.get("label")!r}, {control.get("checked")})',
    )
    win.click_box(control["box"])
    soft_until(lambda: near(width_of(side_box(win)), FOLDED_WIDTH, WIDTH_TOL), 4)
    width = width_of(side_box(win))
    R.check(
        9,
        near(width, FOLDED_WIDTH, WIDTH_TOL),
        f"folded width {FOLDED_WIDTH} +-{WIDTH_TOL} (observed {width})",
    )
    rows = win.rows()
    R.check(
        9,
        rows and all(r.get("label_box") is None for r in rows),
        f"every folded row's label_box null (observed {[r.get('label_box') for r in rows][:3]})",
    )
    R.check(
        9,
        (win.control(FOLD_ID) or {}).get("checked") is False,
        "control 2 unchecked while folded",
    )
    win.click_box((win.control(FOLD_ID) or control)["box"])
    soft_until(lambda: near(width_of(side_box(win)), SIDE_WIDTH, WIDTH_TOL), 4)
    width = width_of(side_box(win))
    R.check(
        9,
        near(width, SIDE_WIDTH, WIDTH_TOL),
        f"unfolded width {SIDE_WIDTH} +-{WIDTH_TOL} (observed {width})",
    )


def tools_gate(win):
    pane = win.landmark_box("pane")
    if pane:
        win.click_box(pane)
    win.key("ctrl+shift+b")
    soft_until(lambda: (win.landmark_box("tools-panel") or [0, 0, 0, 0])[2] > 0, 4)
    box = win.landmark_box("tools-panel")
    right = box[0] + box[2] if box else None
    R.check(
        11,
        box is not None and box[2] > 0 and near(right, WINDOW_W, WIDTH_TOL),
        f"tools-panel right edge {WINDOW_W} +-{WIDTH_TOL} (observed box {box})",
    )
    side = side_box(win)
    R.check(
        11,
        side is not None and side[0] == 0,
        f"side-tabs left edge 0 (observed {side})",
    )
    win.key("ctrl+shift+b")


# ---------------------------------------------------------------- the other windows


def groups_none_and_idle(work):
    config = dict(
        BASE_CONFIG,
        tab_style="vertical",
        tab_groups="none",
        animations=True,
        agents=agents_config(),
    )
    win = window(work, "groups-none", config)
    try:
        win.new_tab()
        soft_until(lambda: len(win.rows_by_tab()) >= 2, 6)
        time.sleep(1.5)
        if wants(7):
            groups = win.landmark_lines("side-tab-group")
            R.check(
                7,
                not groups,
                f"tab_groups none: no side-tab-group landmark (observed {len(groups)})",
            )
            rows = win.rows_by_tab()
            order = [
                line.get("tab")
                for line in sorted(rows.values(), key=lambda line: line.get("index", 0))
            ]
            R.check(
                7,
                order == [0, 1],
                f"tab_groups none: rows in tab order [0, 1] (observed {order})",
            )
            got = [line.get("group", "<missing>") for line in rows.values()]
            R.check(
                7,
                rows and all(g is None for g in got),
                f"tab_groups none: every row's group null (observed {got})",
            )
        if not wants(16):
            return
        R.check(
            16,
            (width_of(side_box(win)) or 0) > 0,
            f"the side column is shown (observed {side_box(win)})",
        )

        def frames():
            return win.frame_count() + win.record_frames()

        win.away()
        time.sleep(2)
        start = frames()
        time.sleep(IDLE_SECONDS)
        R.check(
            16,
            frames() == start,
            f"no working row: 0 new frames in {IDLE_SECONDS} s (observed {frames() - start})",
        )
        win.send(1, "sleep 300\r")
        soft_until(lambda: win.rows_by_tab().get(1, {}).get("state") == "working", 6)
        state = win.rows_by_tab().get(1, {}).get("state")
        R.check(
            16,
            state == "working",
            f"tab 1 running sleep is working (observed {state!r})",
        )
        start = frames()
        time.sleep(IDLE_SECONDS)
        drawn = frames() - start
        R.check(
            16,
            drawn >= SPINNER_MIN_FRAMES,
            f"focused with a working row: at least {SPINNER_MIN_FRAMES} frames in {IDLE_SECONDS} s (observed {drawn})",
        )
        win.unfocus()
        time.sleep(1.5)
        start = frames()
        time.sleep(IDLE_SECONDS)
        R.check(
            16,
            frames() == start,
            f"unfocused: 0 new frames in {IDLE_SECONDS} s (observed {frames() - start})",
        )
    finally:
        win.close()


def bad_config(work):
    config = dict(
        BASE_CONFIG, tab_style="vertical", tab_groups="maybe", agents=agents_config()
    )
    try:
        win = window(work, "bad-config", config)
    except Fail as problem:
        R.check(13, False, f"the window starts ({problem})")
        return
    try:
        R.check(13, True, "the window starts")
        soft_until(lambda: BAD_CONFIG_MESSAGE in win.errors(), 3)
        R.check(
            13,
            BAD_CONFIG_MESSAGE in win.errors(),
            f"stderr holds {BAD_CONFIG_MESSAGE} (observed {win.errors().strip()[-200:]!r})",
        )
        soft_until(
            lambda: side_box(win) is not None and win.landmark_lines("side-tab-group"),
            6,
        )
        box = side_box(win)
        groups = win.landmark_lines("side-tab-group")
        texts = [groups[i].get("text") for i in sorted(groups)]
        if box and box[2] > 0:
            R.check(
                13,
                texts == ["Other"],
                f"groups behave as project: ['Other'] for a tab in ~/HIH (observed {texts})",
            )
        else:
            R.check(
                13,
                box == [0, 0, 0, 0] and not texts,
                f"a window on every default: side-tabs [0,0,0,0], no group header (observed {box}, {texts})",
            )
    finally:
        win.close()


def limits(work):
    log = work / "limits-requests.log"
    config = dict(BASE_CONFIG, tab_style="vertical", agents=agents_config())
    win = window(work, "limits", config, extra_env={"CLANKY_REQUEST_LOG": str(log)})
    try:
        while len(win.workspace()["groups"]) < LIMIT_TABS:
            win.new_tab()
        time.sleep(3)
        side = side_box(win)
        inside = (
            side is not None
            and side[0] >= 0
            and side[1] >= 0
            and side[0] + side[2] <= WINDOW_W
            and side[1] + side[3] <= WINDOW_H
            and side[2] > 0
        )
        R.check(15, inside, f"60 tabs: the column inside the window (observed {side})")
        pane = win.landmark_box("pane")
        pane_ok = (
            pane is not None
            and side is not None
            and pane[2] > 0
            and pane[3] > 0
            and pane[0] >= side[0] + side[2]
            and pane[0] + pane[2] <= WINDOW_W
            and pane[1] + pane[3] <= WINDOW_H
        )
        R.check(
            15,
            pane_ok,
            f"60 tabs: the pane right of the column, inside the window (observed {pane})",
        )
        before = {tab: line["box"] for tab, line in win.rows_by_tab().items()}
        if side:
            win.wheel(side[0] + side[2] / 2, side[1] + side[3] / 2, 5)
            time.sleep(1)
        after = {tab: line["box"] for tab, line in win.rows_by_tab().items()}
        moved = max(
            (before[t][1] - after[t][1] for t in before if t in after), default=0
        )
        R.check(
            15,
            moved >= ROW_HEIGHT,
            f"5 wheel notches move a row up by >= {ROW_HEIGHT} (observed {moved})",
        )
        panes = len(win.panes())

        def ops():
            try:
                return log.read_text(encoding="utf-8").splitlines()
            except OSError:
                return None

        start = ops()
        time.sleep(IDLE_SECONDS)
        end = ops()
        if start is None or end is None:
            R.check(15, False, f"CLANKY_REQUEST_LOG {log.name} is written")
            return
        new = end[len(start) :]
        R.check(
            15,
            len(new) > 0,
            f"the request log grows over {IDLE_SECONDS} s (observed {len(new)} lines)",
        )
        limit = panes * TICKS_IN_IDLE
        for op in ("terminal_info", "input_state"):
            count = sum(1 for line in new if line.strip() == op)
            R.check(
                15,
                count <= limit,
                f"{op}: at most {limit} in {IDLE_SECONDS} s for {panes} panes (observed {count})",
            )
    finally:
        win.close()


def planted_fault():
    env = dict(os.environ, CLANKY_GATE_FAULT="side-width", CLANKY_GPUI_BIN=str(APP))
    progress("planted fault side-width")
    done = subprocess.run(  # noqa: PLW1510 - the exit status is the result
        ["bash", str(REPO / "scripts/gate-s1-side-tabs.sh")],
        cwd=REPO,
        env=env,
        capture_output=True,
        text=True,
    )
    out = done.stdout.strip().splitlines()
    caught = re.compile(
        r"^FAIL 3 vertical: .*side-tabs width: expected 236 \+-0\.5, observed 240(\.0)?\b"
    )
    R.check(
        18,
        done.returncode != 0
        and out
        and out[-1] == "FAIL"
        and any(caught.match(line) for line in out),
        f"the side-width run fails gate 3 on width 240 (exit {done.returncode}: "
        f"{' | '.join(line for line in out if line.startswith('FAIL'))[:300]})",
    )


def guarded(gates, fn, *args):
    if not wants(*gates):
        return
    try:
        fn(*args)
    except Fail as problem:
        R.crash(gates, f"window: {problem}")
    except Exception as problem:  # noqa: BLE001 - a crash still reports FAIL, never PASS
        R.crash(gates, f"{type(problem).__name__}: {problem}")


def main():
    for tool in ("Xvfb", "xdotool", "ffmpeg", "git"):
        if not shutil.which(tool):
            R.crash(list(NAMES), f"{tool} is not installed")
            return
    if not APP.is_file():
        R.crash(list(NAMES), f"no window binary at {APP}")
        return
    work = Path(tempfile.mkdtemp(prefix="clanky-side-tabs-"))
    work.chmod(0o700)
    try:
        if wants(1):
            guarded([1], standards)
        guarded([2], horizontal, work)
        guarded([3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14], vertical, work)
        guarded([7, 16], groups_none_and_idle, work)
        guarded([13], bad_config, work)
        guarded([15], limits, work)
        guarded([17], whole_project, work)
        guarded([18], planted_fault)
    finally:
        if not KEEP:
            shutil.rmtree(work, ignore_errors=True)
        else:
            progress(f"kept {work}")


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # noqa: BLE001 - a crash still reports FAIL, never PASS
        R.crash(list(NAMES), f"{type(e).__name__}: {e}")
    wanted = ONLY or set(NAMES)
    lines = R.lines(wanted)
    for line in lines:
        print(line)
    ok = all(line.startswith("PASS") for line in lines)
    print("PASS" if ok else "FAIL")
    sys.exit(0 if ok else 1)
