#!/usr/bin/env python3
"""The command palette's behaviour on clanky-gpui, read from the control record: today's q1 T3 and q2 K3 palette
steps, retargeted from OCR and fixed coordinates to the record, plus the rest of what today's palette does.

Usage: palette_steps.py [--app PATH] [--fault NAME]

Each step prints "ok: ..." or "FAIL: ...", then PASS or FAIL. Under the gate lock, on the harness's own Xvfb with a
scratch HOME (window-chrome/chrome_harness.py), at scale 1. --fault plants CLANKY_CONTROL_FAULT (a build with the
control-faults feature); palette-no-run must fail P4.

  P1 (q1 T3) a click on the title bar's search box (the main window's field "Search or run a command") opens the
     palette (its screen line with rows and a popover object); typing "Split right" lists "Split right" first, and
     Enter closes it and adds a pane (the owner API lists one more terminal).
  P2 (q2 K3) Ctrl+Shift+P opens it; "Split down" and Enter add a pane.
  P3 Ctrl+Shift+P again while it is open closes it.
  P4 a typed line that names no command: the one row is "Run “echo zq-palette-run” in this pane", and Enter runs it
     in the pane the palette opened over (the pane's text holds zq-palette-run twice: the command and its output).
  P5 with a second tab, "Go to HIH" lists the first tab (both are in the HIH folder), and Enter makes it active.
"""

import os
import shutil
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "window-chrome"))
from chrome_harness import Fail, RealWindow, until  # noqa: E402

REPO = HERE.parents[1]
ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


APP = Path(
    arg("--app", os.environ.get("CLANKY_GPUI_BIN", REPO / "target/release/clanky-gpui"))
)
FAULT = arg("--fault")
CONFIG = dict(
    restore_layout=False,
    new_window_panes=1,
    startup_animation=False,
    history=False,
    animations=False,
    cursor_blink="off",
    theme="dark",
)
SEARCH_BOX = "Search or run a command"
RUN_MARK = "zq-palette-run"
WAIT_S = 5
FAILED = []


def step(name, ok, what, observed):
    if ok:
        print(f"ok: {name} {what}")
    else:
        FAILED.append(name)
        print(f"FAIL: {name} {what}: observed {observed}")


def rows(win):
    line = win.screen("palette") or {}
    return [c["label"] for c in line.get("controls", [])] if line.get("popover") else []


def is_open(win):
    return bool(rows(win))


def terminals(win):
    return len(win.owner.call(1, "list_tabs")["tabs"])


def tabs(win):
    line = win.screen("main-window") or {}
    return [c for c in line.get("controls", []) if c["kind"] == "tab"]


def soft_until(test, seconds=WAIT_S):
    try:
        return until(test, "", seconds)
    except Fail:
        return False


def type_and_pick(win, text, first):
    """Types into the open palette, waits for `first` to lead the list, and presses Enter."""
    win.type(text)
    listed = soft_until(lambda: rows(win)[:1] == [first])
    win.key("Return")
    return listed


def search_box_step(win):
    box = next(
        (
            c
            for c in (win.screen("main-window") or {}).get("controls", [])
            if c["label"] == SEARCH_BOX
        ),
        None,
    )
    if box is None:
        step("P1", False, "the title bar's search box is in the record", "none")
        return
    before = terminals(win)
    x, y, w, h = box["box"]
    win.move(x + w / 2, y + h / 2)
    win.x("click", "1")
    opened = soft_until(lambda: is_open(win))
    step(
        "P1",
        opened and box["kind"] == "field",
        "a click on the search box opens the palette",
        rows(win),
    )
    listed = type_and_pick(win, "Split right", "Split right")
    step(
        "P1",
        listed
        and soft_until(lambda: terminals(win) == before + 1 and not is_open(win)),
        '"Split right" and Enter close it and add a pane',
        f"listed first {listed}, terminals {before} -> {terminals(win)}",
    )


def key_step(win):
    before = terminals(win)
    win.key("ctrl+shift+p")
    opened = soft_until(lambda: is_open(win))
    listed = type_and_pick(win, "Split down", "Split down")
    step(
        "P2",
        opened and listed and soft_until(lambda: terminals(win) == before + 1),
        'Ctrl+Shift+P, "Split down" and Enter add a pane',
        f"opened {opened}, listed first {listed}, terminals {before} -> {terminals(win)}",
    )
    win.key("ctrl+shift+p")
    opened = soft_until(lambda: is_open(win))
    win.key("ctrl+shift+p")
    step(
        "P3",
        opened and soft_until(lambda: not is_open(win)),
        "Ctrl+Shift+P closes the open palette",
        rows(win),
    )


def run_step(win):
    win.key("ctrl+shift+p")
    soft_until(lambda: is_open(win))
    pane = ((win.screen("palette") or {}).get("popover") or {}).get("opener_pane")
    command = f"echo {RUN_MARK}"
    want = [f"Run “{command}” in this pane"]
    win.type(command)
    listed = soft_until(lambda: rows(win) == want)
    step(
        "P4",
        listed,
        "a line that names no command offers to run it",
        f"{rows(win)} for {want}",
    )
    win.key("Return" if listed else "Escape")
    ran = bool(pane) and soft_until(lambda: win.pane_text(pane).count(RUN_MARK) >= 2)
    step("P4", ran, f"Enter runs it in the pane it opened over ({pane})", "no output")


def go_to_step(win):
    win.key("ctrl+shift+t")
    soft_until(lambda: len(tabs(win)) == 2 and tabs(win)[1]["checked"])
    win.key("ctrl+shift+p")
    soft_until(lambda: is_open(win))
    listed = type_and_pick(win, "Go to", "Go to HIH")
    shown = soft_until(lambda: [t["checked"] for t in tabs(win)] == [True, False])
    step(
        "P5",
        listed and shown,
        '"Go to HIH" shows the first tab',
        [(t["label"], t["checked"]) for t in tabs(win)],
    )


def main():
    work = Path(tempfile.mkdtemp(prefix="palette-steps.", dir="/srv/devops/build"))
    win = None
    try:
        win = RealWindow(1.0, work, APP, CONFIG, FAULT)
        until(lambda: tabs(win), "the main window records its tabs", 15)
        time.sleep(1.0)
        steps = (
            [run_step] if FAULT else [search_box_step, key_step, run_step, go_to_step]
        )
        for one in steps:
            one(win)
    except Fail as error:
        FAILED.append("setup")
        print(f"FAIL: setup {error}")
    finally:
        if win:
            win.close()
        shutil.rmtree(work, ignore_errors=True)
    print("FAIL: palette steps" if FAILED else "PASS: palette steps")
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
