#!/usr/bin/env python3
"""Gate for the ways into and out of the Settings window, and the two side-tab rows it changes live.

Usage: gate_open_settings.py [--app PATH] [--fault NAME] [--keep]

Runs clanky-gpui (--app, else $CLANKY_GPUI_BIN, else target/release/clanky-gpui) under Xvfb at scale 1 in the side
tabs harness, with a config.xml the gate writes (tab_style horizontal, tab_groups project), and reads the control
record, the owner API and the file. Expected results, fixed before the run:

  O1 title-bar-button   the title bar's last control (the rightmost of the main-window controls inside the
                        title-bar landmark) is id 3, kind icon-button, label "Settings", its hit box equal to its
                        box, at least 24 logical px wide and high.
  O2 button-toggles     a click on control 3 makes the settings screen line show controls with page "General"
                        within 2 s; a second click makes its controls list empty within 2 s.
  O3 shortcut-escape    after a click on the first pane, the pane with the keyboard is the one pane a typed
                        command runs in (its output 41o3 in that pane's text within 5 s). Ctrl+, opens Settings
                        within 2 s; Escape closes it within 2 s; the workspace's focused pane is the same as before;
                        a command typed next runs in that same pane only (42o3, within 5 s); Ctrl+, then opens
                        Settings and Ctrl+, again closes it, each within 2 s.
  O4 palette-entry      Ctrl+Shift+P, then "settings" typed: the palette's first row (top to bottom) is labelled
                        exactly "Settings"; Enter opens Settings within 2 s.
  O5 appearance-live    on the Appearance page, a click on segment "Side" of row tab_style gives a side-tabs
                        landmark 236 wide and config.xml tab_style "vertical", both within 2 s; with at least one
                        side-tab-group landmark shown, a click on segment "Off" of row tab_groups gives config.xml
                        tab_groups "none" and no side-tab-group landmark with a non-zero box, both within 2 s.

The settings screen counts as open when its last line has a non-empty controls list, and as closed when it has an
empty list or no line at all. A landmark the window takes away is written again with the box [0,0,0,0].

Prints one "PASS On name" or "FAIL On name: why" line per check, then PASS (exit 0) or FAIL (exit 1). With
--fault settings-button-dead (a clanky-gpui built with the control-faults feature) the title-bar button draws but a
click on it does nothing, and O2 must fail.
"""

import os
import shutil
import sys
import tempfile
import time
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "side-tabs"))
sys.path.insert(0, str(HERE.parent / "window-chrome"))
from chrome_harness import Fail, until
from side_window import SideWindow

ARGS = sys.argv[1:]


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


APP = Path(
    arg("--app", os.environ.get("CLANKY_GPUI_BIN", REPO / "target/release/clanky-gpui"))
)
FAULT = arg("--fault")
KEEP = "--keep" in ARGS

BUTTON_ID = 3
BUTTON_LABEL = "Settings"
MIN_HIT_LOGICAL_PX = 24  # the controls gate's least hit box
OPEN_SECONDS = 2.0  # how long Settings, a closing and a live setting change may take
RUN_SECONDS = 5.0  # how long a typed command may take to print
SIDE_WIDTH = 236  # the side column's width at scale 1 (the approved side-tabs mockup)
WHEEL_TRIES = 12  # wheel notches tried to bring a row into view
# Commands whose output differs from the typed line, so a pane's text holds the output only once the command ran.
BEFORE_COMMAND, BEFORE_OUTPUT = "echo $((40+1))o3", "41o3"
MARKER_COMMAND, MARKER_OUTPUT = "echo $((40+2))o3", "42o3"
CONFIG = (
    '<?xml version="1.0"?>\n<clanky version="1">\n'
    "  <tab_style>horizontal</tab_style>\n"
    "  <tab_groups>project</tab_groups>\n"
    "</clanky>\n"
)

results = []


def check(number, name, passed, why=""):
    results.append(passed)
    print(
        f"PASS {number} {name}" if passed else f"FAIL {number} {name}: {why}",
        flush=True,
    )


def wait_for(test, seconds):
    try:
        until(test, "condition", timeout=seconds)
        return True
    except Fail:
        return False


# ---------------------------------------------------------------- reading the record and the file


def settings_line(win):
    return win.screen("settings")


def settings_open(win):
    line = settings_line(win)
    return bool(line and line.get("controls"))


def settings_page(win):
    line = settings_line(win)
    return line.get("page") if line and line.get("controls") else None


def settings_closed(win):
    return not settings_open(win)


def opened_on_general(win):
    return settings_open(win) and settings_page(win) == "General"


def main_controls(win):
    line = win.screen("main-window")
    return line.get("controls", []) if line else []


def main_control(win, ident):
    return next((c for c in main_controls(win) if c.get("id") == ident), None)


def settings_control(win, kind, label, group=None):
    line = settings_line(win)
    for control in line.get("controls", []) if line else []:
        if (
            control.get("kind") == kind
            and control.get("label") == label
            and (group is None or control.get("group") == group)
        ):
            return control
    return None


def live_landmarks(win, name):
    """The landmarks named `name` whose last line has a box with an area: the ones the window shows now."""
    return [
        line
        for line in win.landmark_lines(name).values()
        if line.get("box") and line["box"][2] > 0 and line["box"][3] > 0
    ]


def side_width(win):
    box = win.landmark_box("side-tabs")
    return box[2] if box else None


def config_value(path, key):
    try:
        found = ET.parse(path).getroot().find(key)
    except (ET.ParseError, OSError):
        return None
    return (found.text or "").strip() if found is not None else None


def inside(box, area):
    x, y, w, h = box
    cx, cy = x + w / 2, y + h / 2
    return area[0] <= cx < area[0] + area[2] and area[1] <= cy < area[1] + area[3]


# ---------------------------------------------------------------- acting on the window


def ensure_closed(win):
    """Leaves Settings and any palette closed, so a check starts from the main window whatever came before."""
    for _ in range(2):
        if settings_closed(win):
            break
        win.key("Escape")
        wait_for(lambda: settings_closed(win), OPEN_SECONDS)
    palette = win.screen("palette")
    if palette and palette.get("controls"):
        win.key("Escape")
    win.away()


def click_title_button(win, box):
    win.click_box(box)
    win.away()


def bring_into_view(win, kind, label, group):
    """The settings control, scrolled into the body with the wheel when its line says it is not visible."""
    for _ in range(WHEEL_TRIES):
        control = settings_control(win, kind, label, group)
        if control is None or control.get("visible", True):
            return control
        panel = win.landmark_box("settings-panel")
        if not panel:
            return control
        body_middle = panel[1] + panel[3] / 2
        win.wheel(
            panel[0] + panel[2] / 2,
            body_middle,
            1,
            down=control["box"][1] > body_middle,
        )
        time.sleep(0.2)
    return settings_control(win, kind, label, group)


# ---------------------------------------------------------------- the checks


def title_bar_button(win):
    bar = win.landmark_box("title-bar")
    if not bar:
        check("O1", "title-bar-button", False, "no title-bar landmark in the record")
        return
    in_bar = [c for c in main_controls(win) if c.get("box") and inside(c["box"], bar)]
    if not in_bar:
        check("O1", "title-bar-button", False, f"no control inside the title bar {bar}")
        return
    last = max(in_bar, key=lambda c: (c["box"][0] + c["box"][2], c["box"][0]))
    least = MIN_HIT_LOGICAL_PX * win.scale
    box, hit = last.get("box"), last.get("hit")
    passed = (
        last.get("id") == BUTTON_ID
        and last.get("kind") == "icon-button"
        and last.get("label") == BUTTON_LABEL
        and hit == box
        and box[2] >= least
        and box[3] >= least
    )
    check(
        "O1",
        "title-bar-button",
        passed,
        f"the last title-bar control should be icon-button id {BUTTON_ID} {BUTTON_LABEL!r}, hit equal to box, "
        f"at least {least:g} px each way (observed id {last.get('id')}, kind {last.get('kind')!r}, "
        f"label {last.get('label')!r}, box {box}, hit {hit})",
    )


def button_toggles(win, button):
    ensure_closed(win)
    if button is None or button.get("kind") != "icon-button":
        check(
            "O2",
            "button-toggles",
            False,
            f"no icon-button with id {BUTTON_ID} to click (observed {button})",
        )
        return
    click_title_button(win, button["box"])
    opened = wait_for(lambda: opened_on_general(win), OPEN_SECONDS)
    if not opened:
        check(
            "O2",
            "button-toggles",
            False,
            f"a click on control {BUTTON_ID} did not open Settings on General within {OPEN_SECONDS:g} s "
            f"(settings line {summary(win)})",
        )
        return
    again = main_control(win, BUTTON_ID) or button
    click_title_button(win, again["box"])
    closed = wait_for(lambda: settings_closed(win), OPEN_SECONDS)
    check(
        "O2",
        "button-toggles",
        closed,
        f"a second click on control {BUTTON_ID} did not close Settings within {OPEN_SECONDS:g} s "
        f"(settings line {summary(win)})",
    )


def typed_into(win, command, output):
    """Types `command` and Return; the one pane whose text then shows `output`, or None (none, or more than one)."""
    win.type(command)
    win.key("Return")
    wait_for(
        lambda: any(output in win.pane_text(pane) for pane in win.panes()), RUN_SECONDS
    )
    time.sleep(0.3)
    found = [pane for pane in win.panes() if output in win.pane_text(pane)]
    return found[0] if len(found) == 1 else None


def shortcut_escape(win):
    ensure_closed(win)
    problems = []
    panes = [win.landmark_box("pane", index) for index in (0, 1)]
    if not all(panes):
        check(
            "O3",
            "shortcut-escape",
            False,
            f"the window should start with two panes (pane landmarks {panes})",
        )
        return
    top = panes[0]
    # Into the pane's upper part, clear of the command input along its bottom.
    win.move(top[0] + top[2] / 2, top[1] + top[3] / 4)
    time.sleep(0.15)
    win.x("click", "1")
    time.sleep(0.4)
    win.away()
    # The pane with the keyboard is the one a typed command runs in; the record and the workspace are not asked.
    keyboard_pane = typed_into(win, BEFORE_COMMAND, BEFORE_OUTPUT)
    if keyboard_pane is None:
        check(
            "O3",
            "shortcut-escape",
            False,
            f"after a click on the first pane, a typed command ran in no pane ({BEFORE_OUTPUT} not printed)",
        )
        return
    focused = win.workspace()["groups"][0]["focus"]
    win.key("ctrl+comma")
    if not wait_for(lambda: settings_open(win), OPEN_SECONDS):
        problems.append(f"Ctrl+, did not open Settings within {OPEN_SECONDS:g} s")
    win.key("Escape")
    if not wait_for(lambda: settings_closed(win), OPEN_SECONDS):
        problems.append(f"Escape did not close Settings within {OPEN_SECONDS:g} s")
    after = win.workspace()["groups"][0]["focus"]
    if after != focused:
        problems.append(
            f"the workspace's focused pane was {focused} before and is {after} after"
        )
    ran_in = typed_into(win, MARKER_COMMAND, MARKER_OUTPUT)
    if ran_in != keyboard_pane:
        problems.append(
            f"a command typed after Escape should run in pane {keyboard_pane}, which had the keyboard "
            f"(ran in {ran_in})"
        )
    win.key("ctrl+comma")
    if not wait_for(lambda: settings_open(win), OPEN_SECONDS):
        problems.append(
            f"Ctrl+, from the pane did not open Settings within {OPEN_SECONDS:g} s"
        )
    win.key("ctrl+comma")
    if not wait_for(lambda: settings_closed(win), OPEN_SECONDS):
        problems.append(
            f"Ctrl+, inside Settings did not close it within {OPEN_SECONDS:g} s"
        )
    check("O3", "shortcut-escape", not problems, "; ".join(problems))


def palette_rows(win):
    line = win.screen("palette")
    rows = [
        c
        for c in (line.get("controls", []) if line else [])
        if c.get("kind") == "palette-row"
    ]
    return [c.get("label") for c in sorted(rows, key=lambda c: c["box"][1])]


def palette_entry(win):
    ensure_closed(win)
    start = len(win.records())
    win.key("ctrl+shift+p")
    opened = wait_for(
        lambda: any(
            line.get("record") == "screen"
            and line.get("screen") == "palette"
            and line.get("controls")
            for line in win.records()[start:]
        ),
        OPEN_SECONDS,
    )
    if not opened:
        check("O4", "palette-entry", False, "Ctrl+Shift+P did not open the palette")
        return
    win.type("settings")
    wait_for(lambda: BUTTON_LABEL in palette_rows(win), OPEN_SECONDS)
    labels = palette_rows(win)
    if not labels or labels[0] != BUTTON_LABEL:
        check(
            "O4",
            "palette-entry",
            False,
            f"the first palette row for 'settings' should be {BUTTON_LABEL!r} (observed rows {labels})",
        )
        win.key("Escape")
        return
    win.key("Return")
    opened = wait_for(lambda: settings_open(win), OPEN_SECONDS)
    check(
        "O4",
        "palette-entry",
        opened,
        f"Enter on the palette's {BUTTON_LABEL!r} row did not open Settings within {OPEN_SECONDS:g} s",
    )


def pick_segment(win, key, option):
    """Clicks the segment `option` of row `key`; returns why it could not, or None."""
    control = bring_into_view(win, "segment", option, f"row:{key}")
    if control is None:
        return (
            f"no segment {option!r} in group row:{key} on the {settings_page(win)} page"
        )
    if not control.get("visible", True):
        return f"segment {option!r} of row {key} stays out of view"
    win.click_box(control["box"])
    win.away()
    return None


def appearance_live(win):
    ensure_closed(win)
    path = win.config_path
    win.key("ctrl+comma")
    if not wait_for(lambda: settings_open(win), OPEN_SECONDS):
        check(
            "O5",
            "appearance-live",
            False,
            f"Ctrl+, did not open Settings (side-tabs {win.landmark_box('side-tabs')}, "
            f"config tab_style {config_value(path, 'tab_style')!r})",
        )
        return
    page = settings_control(win, "segment", "Appearance", "settings-pages")
    if page is None:
        check("O5", "appearance-live", False, "no Appearance segment in settings-pages")
        return
    win.click_box(page["box"])
    win.away()
    if not wait_for(lambda: settings_page(win) == "Appearance", OPEN_SECONDS):
        check(
            "O5",
            "appearance-live",
            False,
            f"the Appearance segment did not open its page (page {settings_page(win)!r})",
        )
        return
    problem = pick_segment(win, "tab_style", "Side")
    if problem:
        check("O5", "appearance-live", False, problem)
        return
    side = wait_for(
        lambda: (
            side_width(win) == SIDE_WIDTH
            and config_value(path, "tab_style") == "vertical"
        ),
        OPEN_SECONDS,
    )
    if not side:
        check(
            "O5",
            "appearance-live",
            False,
            f"Side should give a side-tabs landmark {SIDE_WIDTH} wide and tab_style vertical within "
            f"{OPEN_SECONDS:g} s (observed width {side_width(win)}, "
            f"tab_style {config_value(path, 'tab_style')!r})",
        )
        return
    if not wait_for(lambda: live_landmarks(win, "side-tab-group"), OPEN_SECONDS):
        check(
            "O5",
            "appearance-live",
            False,
            "with tab_groups project the side column should show a side-tab-group landmark first (none shown)",
        )
        return
    problem = pick_segment(win, "tab_groups", "Off")
    if problem:
        check("O5", "appearance-live", False, problem)
        return
    regrouped = wait_for(
        lambda: (
            config_value(path, "tab_groups") == "none"
            and not live_landmarks(win, "side-tab-group")
        ),
        OPEN_SECONDS,
    )
    check(
        "O5",
        "appearance-live",
        regrouped,
        f"Off should give tab_groups none and no side-tab-group landmark within {OPEN_SECONDS:g} s "
        f"(observed tab_groups {config_value(path, 'tab_groups')!r}, "
        f"{len(live_landmarks(win, 'side-tab-group'))} group landmarks)",
    )


def summary(win):
    line = settings_line(win)
    if line is None:
        return "absent"
    return f"page {line.get('page')!r}, {len(line.get('controls') or [])} controls"


# ---------------------------------------------------------------- running it


def write_config(folder):
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / "config.xml"
    path.write_text(CONFIG, encoding="utf-8")
    return path


def main():
    work = Path(tempfile.mkdtemp(prefix="clanky-open-settings-"))
    names = {
        "O1": "title-bar-button",
        "O2": "button-toggles",
        "O3": "shortcut-escape",
        "O4": "palette-entry",
        "O5": "appearance-live",
    }
    try:
        try:
            win = SideWindow(
                work, APP, {}, fault=FAULT, keep=KEEP, config_file=write_config
            )
        except Fail as error:
            for number, name in names.items():
                check(number, name, False, f"the window did not start: {error}")
            win = None
        if win is not None:
            try:
                until(lambda: win.screen("main-window"), "the window draws", 20)
                time.sleep(1)
                steps = (
                    ("O1", lambda: title_bar_button(win)),
                    ("O2", lambda: button_toggles(win, main_control(win, BUTTON_ID))),
                    ("O3", lambda: shortcut_escape(win)),
                    ("O4", lambda: palette_entry(win)),
                    ("O5", lambda: appearance_live(win)),
                )
                for number, step in steps:
                    try:
                        step()
                    except (Fail, KeyError, IndexError, TypeError) as error:
                        check(number, names[number], False, f"stopped: {error!r}")
            finally:
                win.close()
    finally:
        if not KEEP:
            shutil.rmtree(work, ignore_errors=True)
    passed = all(results) and len(results) == len(names)
    print("PASS" if passed else "FAIL")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
