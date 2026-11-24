#!/usr/bin/env python3
"""Gate for the command palette's tab switch ("Switch to side tabs" / "Switch to top tabs").

Usage: gate_tab_switch.py [--app PATH] [--fault NAME] [--keep]

Runs clanky-gpui under Xvfb at scale 1 with a config.xml the gate writes, picks the switch from the palette
(Ctrl+Shift+P, typed words, Enter) and reads the control record and the file. Expected results, fixed before the run:

  P1 top tabs: the palette lists "Switch to side tabs" and no "Switch to top tabs".
  P2 picking it: the side-tabs landmark is 236 wide within 5 s, and config.xml holds tab_style "vertical" with every
     other setting it had (font_size 18, tab_groups none, theme_dark graphite-dark, one agent "Pi" running "pi")
     unchanged.
  P3 "Switch to top tabs": the side-tabs landmark is [0,0,0,0] within 5 s, config.xml holds tab_style "horizontal"
     and the other settings, and its mode is 0600.
  P4 no settings file yet, the window started without --config: picking the switch creates the default one,
     $XDG_CONFIG_HOME/clanky/config.xml, holding only tab_style "vertical".
  P5 a settings file that does not parse: the window still switches (width 236), the file's bytes are unchanged, and
     the status line's notice says "not saved".

Prints one "PASS Pn name" or "FAIL Pn name: why" line per check, then PASS (exit 0) or FAIL (exit 1). With
--fault tab-switch-unsaved (a clanky-gpui built with the control-faults feature) the switch saves nothing, and P2
must fail.
"""

import shutil
import stat
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "window-chrome"))
from chrome_harness import Fail, until
from side_window import SideWindow

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


APP = Path(arg("--app", REPO / "target/release/clanky-gpui"))
FAULT = arg("--fault")
KEEP = "--keep" in ARGS
SIDE_WIDTH = 236
HEAD = '<?xml version="1.0"?>\n<clanky version="1">\n'
OTHERS = (
    "  <font_size>18</font_size>\n"
    "  <tab_groups>none</tab_groups>\n"
    "  <theme_dark>graphite-dark</theme_dark>\n"
    "  <agents>\n    <agent><name>Pi</name><command>pi</command></agent>\n  </agents>\n"
)
OTHER_SETTINGS = {
    "font_size": "18",
    "tab_groups": "none",
    "theme_dark": "graphite-dark",
    "agents": (("agent", (("name", "Pi"), ("command", "pi"))),),
}
BROKEN = HEAD + "  <font_size>18</font_sizee>\n</clanky>\n"

results = []


def check(number, name, passed, why=""):
    results.append(passed)
    print(
        f"PASS {number} {name}" if passed else f"FAIL {number} {name}: {why}",
        flush=True,
    )


def canonical(element):
    """An element's settings as plain values: a scalar's text, or its children's (tag, value) pairs."""
    if len(element) == 0:
        return (element.text or "").strip()
    return tuple((child.tag, canonical(child)) for child in element)


def settings(path):
    return {child.tag: canonical(child) for child in ET.parse(path).getroot()}


def side_box(win):
    return win.landmark_box("side-tabs")


def side_width(win):
    box = side_box(win)
    return box[2] if box else None


def palette_labels(win):
    line = win.screen("palette")
    controls = line.get("controls", []) if line else []
    return [c["label"] for c in controls if c.get("kind") == "palette-row"]


def palette_pick(win, words, label):
    """Opens the palette, types `words`, checks `label` is listed, and picks the first row; returns the labels."""
    win.key("ctrl+shift+p")
    until(lambda: win.screen("palette") is not None, "palette opens")
    win.type(words)
    until(lambda: label in palette_labels(win), f"palette lists {label!r}")
    labels = palette_labels(win)
    win.key("Return")
    return labels


def wait_for(test, seconds=5.0):
    try:
        until(test, "condition", timeout=seconds)
        return True
    except Fail:
        return False


def start(work, name, write):
    def config_file(folder):
        return write(folder)

    win = SideWindow(
        work / name, APP, {}, fault=FAULT, keep=KEEP, config_file=config_file
    )
    until(lambda: win.screen("main-window") is not None, "the window draws")
    return win


def write_good(folder):
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / "config.xml"
    path.write_text(
        HEAD + "  <tab_style>horizontal</tab_style>\n" + OTHERS + "</clanky>\n"
    )
    return path


def run_switch_both_ways(work):
    win = start(work, "both-ways", write_good)
    try:
        path = win.config_path
        labels = palette_pick(win, "switch to", "Switch to side tabs")
        check(
            "P1",
            "palette-lists-side",
            "Switch to side tabs" in labels and "Switch to top tabs" not in labels,
            f"labels {labels}",
        )
        wide = wait_for(lambda: side_width(win) == SIDE_WIDTH)
        saved = settings(path)
        expected = dict(OTHER_SETTINGS, tab_style="vertical")
        check(
            "P2",
            "switch-to-side",
            wide and saved == expected,
            f"side width {side_width(win)}, file {saved}",
        )
        palette_pick(win, "switch to", "Switch to top tabs")
        gone = wait_for(lambda: side_box(win) == [0, 0, 0, 0])
        saved = settings(path)
        mode = stat.S_IMODE(path.stat().st_mode)
        check(
            "P3",
            "switch-to-top",
            gone
            and saved == dict(OTHER_SETTINGS, tab_style="horizontal")
            and mode == 0o600,
            f"side box {side_box(win)}, file {saved}, mode {oct(mode)}",
        )
    finally:
        win.close()


def run_missing_file(work):
    def no_config(_folder):
        return None

    win = start(work, "missing", no_config)
    path = win.work / "config" / "clanky" / "config.xml"
    try:
        palette_pick(win, "switch to", "Switch to side tabs")
        made = wait_for(lambda: path.exists() and side_width(win) == SIDE_WIDTH)
        saved = settings(path) if path.exists() else None
        check(
            "P4",
            "missing-file",
            made and saved == {"tab_style": "vertical"},
            f"file {saved}",
        )
    finally:
        win.close()


def run_broken_file(work):
    def broken(folder):
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / "config.xml"
        path.write_text(BROKEN)
        return path

    win = start(work, "broken", broken)
    try:
        palette_pick(win, "switch to", "Switch to side tabs")
        wide = wait_for(lambda: side_width(win) == SIDE_WIDTH)
        notice = win.landmark_lines("notice").get(0, {}).get("text", "")
        unchanged = win.config_path.read_text() == BROKEN
        check(
            "P5",
            "broken-file",
            wide and unchanged and "not saved" in notice,
            f"side width {side_width(win)}, unchanged {unchanged}, notice {notice!r}",
        )
    finally:
        win.close()


def main():
    work = Path(tempfile.mkdtemp(prefix="clanky-tab-switch-"))
    try:
        for run in (run_switch_both_ways, run_missing_file, run_broken_file):
            try:
                run(work)
            except Fail as error:
                check("P?", run.__name__, False, str(error))
    finally:
        if not KEEP:
            shutil.rmtree(work, ignore_errors=True)
    passed = all(results) and len(results) == 5
    print("PASS" if passed else "FAIL")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
