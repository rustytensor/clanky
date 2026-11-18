#!/usr/bin/env python3
"""Editor pane gate, window part: today's editor pane in clanky-gpui, judged from the control record, bytes on disk,
screenshots and the frame trace.

Usage: check_editor_pane.py --app BINARY [--fault NAME] [--speed] [--keep]
(--speed runs the speed check alone.)

Runs one clanky-gpui (built with the control-faults feature, so a fault can be planted with CLANKY_CONTROL_FAULT) on a
private Xvfb at scale 1, 2000 x 1000 logical pixels, one terminal pane, the Graphite dark theme, cursor blink off,
with the window chrome gates' harness (scratch HOME and XDG folders, no Wayland, no session bus). The shell starts in
~/HIH; the gate copies its fixtures there and clicks them in the Files tree, as a person would.

Expected results, fixed before the pane was built (today's behaviour, crates/clanky/src/editor.rs and
editor_workspace.rs, read through Larra; clanky_edit undoes one edit at a time):
  open    a click on edit.txt in the Files tree opens an editor pane: the main window records the controls Save and
          "Close ~/HIH/edit.txt", and the `editor-pane` landmark lies right of the terminal pane's landmark (beside it,
          same top within 1 px), so it opened next to the focused pane.
  marker  edit.txt holds "alpha\\nbeta\\n". Typing "zq" at the caret (line 1, column 1) shows the pending-change
          marker (the `editor-unsaved` landmark gets a width > 0) and leaves the file on disk as it was.
  undo    Ctrl+Z, then Shift+End and "X": Ctrl+Z takes back the "q" alone and puts the caret back where it was before
          that edit, after "z" (`Document::undo` restores the edit's `before` selection), so Shift+End selects "alpha"
          and X replaces it.
  save    Ctrl+S writes exactly "zX\\nbeta\\n" to the file, and the marker goes (width 0).
  close   Typing "y" (pending again) then Ctrl+Shift+W leaves the pane open (Save still recorded), the marker shown
          and the file unchanged ("zX\\nbeta\\n"): closing with pending changes is refused, as today. Ctrl+Z, then
          Ctrl+Shift+W, closes it: the window records no Save any more.
  idle    0 frames drawn from 1 s to 4 s after the last key (the design's "no idle frames"; nothing blinks).
  syntax  cursor_blink.rs (tests/syntax, the syntax gate's fixture) opened from the Files tree with the tools panel then
          hidden (Ctrl+Shift+B), so its 88-column lines fit: every token the syntax gate's table judges for that file is
          drawn in its class colour from assets/themes/graphite-dark.json, with the syntax gate's own judging (marker
          blocks for the grid, colour fit, residual <= 3, at least 90 % of tokens judged).
  speed   (--speed) a 10,000-line Rust file written here: 60 letters typed at 100 ms intervals at line 5,000 after
          Page Down to it; the frame trace's key-to-present p99 is at most G1's limit, today's key p99 median plus
          2 ms (scripts/terminal-pane/baseline.json, as speed_verdict.py judges it), and 0 frames 1 s to 4 s after.

Planted faults (CLANKY_CONTROL_FAULT, each must fail its check): save-not-written (save), marker-stays (save),
syntax-off-by-one (syntax), close-drops-pending (close).
"""

import importlib.util
import json
import math
import os
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(REPO / "scripts/window-chrome"))
import chrome_harness  # noqa: E402

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


APP = Path(arg("--app", os.environ.get("CLANKY_GPUI_BIN", "")))
FAULT = arg("--fault")
SPEED = "--speed" in ARGS
KEEP = "--keep" in ARGS
WORK = Path(os.environ.get("CLANKY_EDITOR_WORK", "/srv/devops/build/clanky-editor-gate-run"))
WIDE, TALL = 2000, 1000
THEME = "graphite-dark"
BASELINE = REPO / "scripts/terminal-pane/baseline.json"
KEY_SLACK_MS = 2
CONFIG = dict(
    restore_layout=False,
    new_window_panes=1,
    startup_animation=False,
    history=False,
    animations=False,
    cursor_blink="off",
    theme="dark",
)
EDIT_BEFORE = "alpha\nbeta\n"
EDIT_SAVED = "zX\nbeta\n"

# The syntax gate's judging and its expected classes (worked out before Q3 from tree-sitter's own parse of the
# fixture); it reads CARGO_TARGET_DIR at import only to name a binary it is not asked to run here.
os.environ.setdefault("CARGO_TARGET_DIR", str(WORK / "unused-target"))
os.environ["CLANKY_Q3_WORK"] = str(WORK / "q3")
_spec = importlib.util.spec_from_file_location("syntax_gate", REPO / "scripts/gate-q3-syntax.py")
syntax_gate = importlib.util.module_from_spec(_spec)
_argv, sys.argv = sys.argv, [sys.argv[0]]
_spec.loader.exec_module(syntax_gate)
sys.argv = _argv
syntax_gate.W, syntax_gate.H = WIDE, TALL

RESULTS = []


def result(name, ok, detail):
    RESULTS.append((name, ok))
    print(f"{'ok' if ok else 'FAIL'}: {name} {detail}")


class Stop(Exception):
    """A check failed; the checks after it depend on its state and are not run."""


def need(name, ok, detail):
    result(name, ok, detail)
    if not ok:
        raise Stop(name)


def controls(win):
    return (win.screen("main-window") or {}).get("controls", [])


def control(win, label, kind=None):
    return next(
        (c for c in controls(win) if c["label"] == label and (kind is None or c["kind"] == kind)), None
    )


def landmark(win, name, index=0):
    boxes = win.landmarks().get(name, [])
    return boxes[index] if len(boxes) > index else None


def click(win, box):
    x, y, w, h = box
    win.move(x + w // 2, y + h // 2)
    win.x("click", "1")
    time.sleep(0.4)


def wait(condition, timeout=4.0):
    try:
        return chrome_harness.until(condition, "", timeout)
    except chrome_harness.Fail:
        return None


def marker_width(win, slot):
    box = landmark(win, "editor-unsaved", slot)
    return box[2] if box else None


def open_from_tree(win, name):
    """Clicks a file's row in the Files tree; the row appears with the window's next look at the folder."""
    row = wait(lambda: control(win, name, "list-row"), 6)
    if row is None:
        return False
    win.away()
    click(win, row["box"])
    return wait(lambda: control(win, "Save"), 4) is not None


def edit_story(win, home):
    path = home / "edit.txt"
    path.write_text(EDIT_BEFORE)
    need("open", open_from_tree(win, "edit.txt"), "edit.txt from the Files tree: Save recorded")
    close = control(win, "Close ~/HIH/edit.txt", "icon-button")
    pane, editor = landmark(win, "pane"), landmark(win, "editor-pane")
    beside = bool(pane and editor and editor[0] >= pane[0] + pane[2] and abs(editor[1] - pane[1]) <= 1)
    need("open", close is not None and beside, f"Close named for the file, editor {editor} right of pane {pane}")
    win.type("zq")
    shown = wait(lambda: (marker_width(win, 0) or 0) > 0)
    need("marker", bool(shown) and path.read_text() == EDIT_BEFORE,
         f"after typing: marker width {marker_width(win, 0)}, file {path.read_text()!r}")
    win.key("ctrl+z", "shift+End")
    win.type("X")
    win.key("ctrl+s")
    saved = wait(lambda: path.read_text() == EDIT_SAVED, 3)
    need("save", bool(saved), f"file after Ctrl+S {path.read_text()!r}, expected {EDIT_SAVED!r}")
    gone = wait(lambda: marker_width(win, 0) == 0, 3)
    need("save", bool(gone), f"marker after save: width {marker_width(win, 0)}, expected 0")
    win.type("y")
    wait(lambda: (marker_width(win, 0) or 0) > 0)
    win.key("ctrl+shift+w")
    time.sleep(1)
    kept = control(win, "Save") is not None and (marker_width(win, 0) or 0) > 0
    need("close", kept and path.read_text() == EDIT_SAVED,
         f"Ctrl+Shift+W with a pending change: pane kept {control(win, 'Save') is not None}, "
         f"marker {marker_width(win, 0)}, file {path.read_text()!r}")
    before = win.frame_count()
    time.sleep(1)
    start = win.frame_count()
    time.sleep(3)
    idle = win.frame_count() - start
    result("idle", idle == 0, f"{idle} frames 1-4 s after the last key ({start - before} in the first second)")
    win.key("ctrl+z", "ctrl+shift+w")
    closed = wait(lambda: control(win, "Save") is None, 3)
    need("close", bool(closed) and path.read_text() == EDIT_SAVED,
         f"Ctrl+Z then Ctrl+Shift+W closes the clean pane: closed {bool(closed)}, file {path.read_text()!r}")


def syntax_story(win, home):
    name = "cursor_blink.rs"
    shutil.copy(syntax_gate.FIX / name, home / name)
    need("syntax", open_from_tree(win, name), f"{name} opens from the Files tree")
    win.key("ctrl+shift+b")
    time.sleep(1)
    win.away()
    geo = wait(lambda: syntax_gate.geometry(win.shot("syntax")), 6)
    need("syntax", geo is not None, "the fixture's four marker blocks give the grid")
    lines = (syntax_gate.FIX / name).read_text().split("\n")
    palette = syntax_gate.theme_colours(THEME)
    try:
        judged = syntax_gate.check_tokens(win.shot("syntax"), geo, name, lines, syntax_gate.EXPECT[name], palette, THEME)
    except syntax_gate.Fail as failure:
        need("syntax", False, str(failure))
    result("syntax", True, f"{name}: {judged} tokens in their {THEME} colours")
    win.key("ctrl+shift+w")
    wait(lambda: control(win, "Save") is None, 3)
    win.key("ctrl+shift+b")
    time.sleep(1)


def big_file(path):
    """10,000 lines of Rust, the same every run."""
    lines = [f"fn line_{n}() -> u32 {{ let value = {n} * 7; value + 1 }} // line {n}" for n in range(10_000)]
    path.write_text("\n".join(lines) + "\n")


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, math.ceil(share * len(ordered)) - 1)]


def speed_story(win, home):
    big_file(home / "big.rs")
    need("speed", open_from_tree(win, "big.rs"), "big.rs opens from the Files tree")
    for _ in range(5):
        win.x("key", "--delay", "20", *(["Page_Down"] * 60))
    time.sleep(1)
    trace = win.work / "frames.csv"
    seen = len(trace.read_text().splitlines())
    win.x("type", "--delay", "100", "abcdefghij" * 6)
    time.sleep(1)
    lines = trace.read_text().splitlines()[seen:]
    latencies = [int(line.split(",")[4]) / 1000 for line in lines if line.startswith("latency,")]
    baseline = json.loads(BASELINE.read_text())["numbers"]["key_p99_ms"]["median"]
    limit = baseline + KEY_SLACK_MS
    p99 = percentile(latencies, 0.99) if latencies else math.inf
    result("speed", len(latencies) >= 50 and p99 <= limit,
           f"{len(latencies)} keys, key to frame p99 {p99:.2f} ms (limit {limit:.2f} ms)")
    start = win.frame_count()
    time.sleep(3)
    idle = win.frame_count() - start
    result("speed", idle == 0, f"{idle} frames 1-4 s after typing in the 10,000-line file")
    win.key("ctrl+z")


def main():
    if not APP.is_file():
        result("setup", False, f"no clanky-gpui at {APP}")
        return
    if WORK.exists():
        shutil.rmtree(WORK)
    WORK.mkdir(mode=0o700, parents=True)
    chrome_harness.LOGICAL_W, chrome_harness.LOGICAL_H = WIDE, TALL
    win = chrome_harness.RealWindow(1, WORK, APP, CONFIG, FAULT, KEEP)
    try:
        home = win.work / "home/HIH"
        if SPEED:
            stories = [speed_story]
        elif FAULT == "syntax-off-by-one":
            stories = [syntax_story]
        else:
            stories = [edit_story, syntax_story]
        for story in stories:
            try:
                story(win, home)
            except Stop:
                break
    finally:
        win.close()
        if not KEEP:
            shutil.rmtree(WORK, ignore_errors=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # a crash still reports FAIL, never PASS
        result("gate", False, f"crashed: {type(e).__name__}: {e}")
    if RESULTS and all(ok for _, ok in RESULTS):
        print("PASS: editor pane window checks")
        sys.exit(0)
    sys.exit(1)
