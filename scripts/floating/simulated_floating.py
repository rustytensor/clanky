"""The simulated window of G3: the main window of window-chrome/simulated_window.py plus the floating screens.

Ctrl+Shift+P opens the palette, Ctrl+Shift+G the agent picker, Ctrl+Shift+F the find bar, Tab after `ls scripts/gate-m` typed
into the pane the completion list, a right click the context menu. Esc closes the open one and gives the focus back to
what had it. Lists filter as text is typed, arrows move the highlighted row, Enter picks it, Tab stays inside, except in
the completion list, where Tab with no row focused puts the highlighted row in and closes it, as today's input does. The
planted faults (`fault`) are the four of G3's definition: popover-over-opener, esc-loses-focus, palette-row-short,
idle-draw, plus tab-traps-completion (the completion list keeps Tab inside); the faults of the chrome simulation still work.
"""

import json
import sys
import time
from pathlib import Path

import numpy as np

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(SCRIPT_DIR.parent / "window-chrome"))
import draw_chrome_reference as chrome_draw  # noqa: E402
import draw_floating as floating_draw  # noqa: E402
import expected_floating  # noqa: E402
import simulated_window as chrome_sim  # noqa: E402
from chrome_harness import latest_landmarks  # noqa: E402

FLOATING_FAULTS = (
    "popover-over-opener",
    "esc-loses-focus",
    "palette-row-short",
    "idle-draw",
    "palette-text-big",
    "find-count-off",
    "find-mark-accent",
    "shift-enter-next",
    "find-field-short",
    "tab-traps-completion",
)
POPOVER_SCREENS = (
    "context-menu",
    "palette",
    "agent-picker",
    "completion-list",
    "find-bar",
)
FIND_TEXT = "g1 g1 g1"
FIND_ROW_TOP = 264  # logical px: the row of pane text the find bar searches
IDLE_FRAMES_PER_SECOND = 30  # what the idle-draw fault draws
COMPLETION_TRIGGER = "ls scripts/gate-m"


class SimFloatingWindow(chrome_sim.SimWindow):
    def __init__(self, scale, theme="graphite-dark", fault=None):
        self.float_fault = fault if fault in FLOATING_FAULTS else None
        self.expected = expected_floating.build(scale, theme)
        self.return_focus = None
        # True after the esc-loses-focus fault closed a popover: the keyboard went nowhere, so typing reaches no pane.
        self.pane_deaf = False
        self.find_index = 0
        self.frames = 0
        self.frames_started = time.monotonic()
        super().__init__(scale, theme, None if self.float_fault else fault)
        self.expected = expected_floating.build(scale, theme)
        self._find_row = self._find_row_box()
        self._record()

    # -- layout helpers ----------------------------------------------------------------------------------------

    def _find_row_box(self):
        s = self.scale
        px = self.expected["cap_heights"]["text_ui"]["size_px"] * s
        height = chrome_draw.round_half_up(px * 1.2)
        return [
            chrome_draw.round_half_up((self.layout.content_left + 16) * s),
            chrome_draw.round_half_up(FIND_ROW_TOP * s),
            chrome_draw.round_half_up(300 * s),
            height,
        ]

    def _opener_field(self):
        return next(c for c in self.layout.controls if c["kind"] == "field")

    def _find_matches(self, query):
        """Device-pixel boxes of every occurrence of `query` in the find row's text."""
        if not query:
            return []
        s = self.scale
        px = self.expected["cap_heights"]["text_ui"]["size_px"] * s
        boxes, start = [], 0
        while (at := FIND_TEXT.lower().find(query.lower(), start)) >= 0:
            left = self._find_row[0] + chrome_draw.text_width(FIND_TEXT[:at], 400, px)
            width = chrome_draw.text_width(FIND_TEXT[at : at + len(query)], 400, px)
            boxes.append(
                [
                    chrome_draw.round_half_up(left),
                    self._find_row[1],
                    chrome_draw.round_half_up(width),
                    self._find_row[3],
                ]
            )
            start = at + 1
        return boxes

    # -- opening -----------------------------------------------------------------------------------------------

    def _open(self, popover):
        """Keeps what had the focus, opens the popover, and gives it no focus of its own until a key moves it."""
        if not self.popover:
            self.return_focus = self.focus_id
        popover["opener"] = popover.get("opener", [self.pw // 2, 0, 0, 0])
        self.popover = popover
        self.pane_deaf = False
        self.focus_id = None
        self.keyboard = False

    def _open_palette(self, screen, master, placeholder, footer):
        left = (
            chrome_draw.LOGICAL_W
            - (
                floating_draw.PALETTE_WIDTH
                if screen == "palette"
                else floating_draw.AGENT_WIDTH
            )
        ) / 2
        width = (
            floating_draw.PALETTE_WIDTH
            if screen == "palette"
            else floating_draw.AGENT_WIDTH
        )
        popover = floating_draw.list_popover(
            self.expected,
            screen,
            master,
            "",
            left,
            72,
            width,
            footer,
            placeholder,
            short_row=self.float_fault == "palette-row-short" and screen == "palette",
        )
        popover["master"] = master
        self._open(popover)

    def _open_completion(self):
        opener = list(self._opener_field()["box"])
        rows = floating_draw.filtered(floating_draw.COMPLETIONS, "")
        popover = floating_draw.completion_popover(
            self.expected,
            opener,
            (self.pw, self.ph),
            rows,
            self.float_fault == "popover-over-opener",
        )
        popover["opener"] = opener
        popover["master"] = floating_draw.COMPLETIONS
        self._open(popover)

    def _open_find(self):
        popover = floating_draw.find_popover(self.expected, self.layout.panes[0], "")
        popover["master"] = None
        if self.float_fault == "find-field-short":
            popover["items"][0]["box"][3] -= 2
        self._open(popover)

    def _open_menu(self):
        before = self.focus_id
        super()._open_menu()
        self.popover["screen"] = "context-menu"
        self.popover["input"] = self.popover["footer"] = None
        self.return_focus = before

    # -- the record --------------------------------------------------------------------------------------------

    def _record(self):
        states = self._states()
        main = self.layout.record_lines("main-window", states)[-1]
        main["focus"] = (
            self.focus_id if self._in_main_window(self.focus_id) else self._main_focus()
        )
        main["keyboard"] = self.keyboard
        lines = [main]
        for name in POPOVER_SCREENS:
            lines.append(self._popover_line(name, states))
        for name, box in self._landmarks_now():
            lines.append({"record": "landmark", "name": name, "index": 0, "box": box})
        count = self._find_count()
        count_box = list(self.popover["items"][0]["box"]) if count else [0, 0, 0, 0]
        lines.append(
            {
                "record": "landmark",
                "name": "find-count",
                "index": 0,
                "box": count_box,
                "text": count,
            }
        )
        for line in lines:
            key = (
                line["screen"]
                if line["record"] == "screen"
                else f"landmark {line['name']}"
            )
            text = json.dumps(line, sort_keys=True)
            if self._last.get(key) != text:
                self._last[key] = text
                self._records.append(line)
                self.frames += 1

    def _main_focus(self):
        """The main window's focus while a popover is open: what had it when the popover opened."""
        return self.return_focus if self.popover else None

    def _popover_line(self, name, states):
        line = {
            "record": "screen",
            "screen": name,
            "scale": self.scale,
            "focus": None,
            "keyboard": self.keyboard,
            "popover": None,
            "controls": [],
        }
        if self.popover and self.popover["screen"] == name:
            line["popover"] = {
                "box": self.popover["box"],
                "opener": self.popover["opener"],
                "opener_id": None,
                "opener_pane": "1",
            }
            line["focus"] = self.focus_id
            line["controls"] = [
                self._popover_control(item, states) for item in self.popover["items"]
            ]
        return line

    def _popover_control(self, item, states):
        return {
            "id": item["id"],
            "kind": item["kind"],
            "label": item["label"],
            "box": item["box"],
            "hit": item["box"],
            "state": states[item["id"]],
            "enabled": True,
            "tab": True,
            "group": item.get("group"),
            "axis": item.get("axis"),
            "checked": None,
            "primary": False,
            "label_box": item.get("label_box"),
            "icon": item.get("icon"),
        }

    def _landmarks_now(self):
        marks = list(floating_draw.landmarks_of(self.popover)) if self.popover else []
        if not self.popover:
            marks += [("popover-input", [0, 0, 0, 0]), ("popover-footer", [0, 0, 0, 0])]
        marks.append(("find-match", self._current_match() or [0, 0, 0, 0]))
        return marks

    def text_count(self, text):
        """How many times `text` stands in the pane: the find row's text."""
        return FIND_TEXT.count(text)

    def _find_count(self):
        """The find bar's count, as today's window words it; the find-count-off fault counts one too many."""
        if not self.popover or self.popover["screen"] != "find-bar":
            return ""
        query = self.popover["query"]
        total = len(self._find_matches(query)) + (self.float_fault == "find-count-off")
        if not query:
            return ""
        if total == 0:
            return "No matches"
        return "1 match" if total == 1 else f"{total} matches"

    def _current_match(self):
        if not self.popover or self.popover["screen"] != "find-bar":
            return None
        matches = self._find_matches(self.popover["query"])
        return matches[self.find_index % len(matches)] if matches else None

    def _activate(self, control, via):
        screen = self.popover["screen"] if self.popover else "main-window"
        self._records.append(
            {
                "record": "activate",
                "screen": screen,
                "id": control["id"],
                "label": control["label"],
                "via": via,
            }
        )
        if screen == "find-bar":
            self.find_index += {"Next": 1, "Previous": -1}.get(control["label"], 0)
            if control["label"] == "Close":
                self._close()
        elif self.popover:
            self._close()

    def _close(self):
        """Closes the popover and gives the focus back to what had it (the fault leaves it nowhere)."""
        self.popover = None
        self.focus_id = (
            None if self.float_fault == "esc-loses-focus" else self.return_focus
        )
        self.keyboard = self.focus_id is not None
        self.pane_deaf = self.float_fault == "esc-loses-focus"

    # -- input -------------------------------------------------------------------------------------------------

    def _key(self, name):
        if name == "ctrl+shift+p":
            self._open_palette(
                "palette",
                floating_draw.PALETTE_COMMANDS,
                floating_draw.PALETTE_PLACEHOLDER,
                True,
            )
        elif name == "ctrl+shift+g":
            self._open_palette(
                "agent-picker",
                floating_draw.AGENTS,
                floating_draw.AGENT_PLACEHOLDER,
                False,
            )
        elif name == "ctrl+shift+f":
            self._open_find()
        elif (
            name == "Tab"
            and not self.popover
            and self.typed.endswith(COMPLETION_TRIGGER)
        ):
            self._open_completion()
        elif name == "ctrl+u" and not self.popover:
            self.typed = ""
        elif name == "Escape" and self.popover:
            self._close()
        elif (
            name in ("Return", "shift+Return")
            and self.popover
            and self.popover["screen"] == "find-bar"
            and self.focus_id is None
        ):
            backwards = (
                name == "shift+Return" and self.float_fault != "shift-enter-next"
            )
            self.find_index += -1 if backwards else 1
        elif (
            name == "Tab"
            and self.popover
            and self.popover["screen"] == "completion-list"
            and self.focus_id is None
            and self.float_fault != "tab-traps-completion"
        ):
            self._close()  # Tab puts the highlighted row in, as today's input does
        elif self.popover and name in ("Tab", "shift+Tab"):
            self._step(self._order(), name == "Tab")
        else:
            super()._key(name)

    def type(self, text):
        popover = self.popover
        if (
            popover
            and popover.get("master")
            and popover["screen"] in ("palette", "agent-picker", "completion-list")
        ):
            self._retype(
                popover,
                popover["query"] + text if popover.get("query") is not None else text,
            )
        elif popover and popover["screen"] == "find-bar":
            popover["query"] += text
            popover["items"][0]["label"] = popover["query"]
            self.find_index = 0
        elif not (popover is None and self.pane_deaf):
            super().type(text)
        self._record()

    def _retype(self, popover, query):
        """Re-lists a filtering popover for the new query: the same ids for the same entries, fewer rows."""
        screen = popover["screen"]
        if screen == "completion-list":
            return
        left = popover["box"][0] / self.scale
        width = popover["box"][2] / self.scale
        new = floating_draw.list_popover(
            self.expected,
            screen,
            popover["master"],
            query,
            left,
            popover["box"][1] / self.scale,
            width,
            popover["footer"] is not None,
            popover["placeholder"],
            short_row=self.float_fault == "palette-row-short" and screen == "palette",
        )
        new["master"], new["opener"] = popover["master"], popover["opener"]
        self.popover = new
        self.focus_id = None

    def frame_count(self):
        """Frames drawn so far; the idle-draw fault draws on without being asked."""
        extra = 0
        if self.float_fault == "idle-draw" and self.popover:
            extra = int(
                (time.monotonic() - self.frames_started) * IDLE_FRAMES_PER_SECOND
            )
        return self.frames + extra

    # -- what the gates read -----------------------------------------------------------------------------------

    def landmarks(self):
        return latest_landmarks(self._records)

    def shot(self, name):
        popover, self.popover = (
            self.popover,
            None,
        )  # the chrome simulation would draw its own menu
        try:
            image = super().shot(name)
        finally:
            self.popover = popover
        canvas = chrome_draw.Canvas(self.pw, self.ph, 0)
        canvas.array = image.astype(float)
        states = self._states()
        match = self._current_match()
        if match:
            # The current match takes the selection colour; the find-mark-accent fault paints it in accent.
            mark = "accent" if self.float_fault == "find-mark-accent" else "selection"
            canvas.rect(match, self.theme.rgb[mark])
        px = self.expected["cap_heights"]["text_ui"]["size_px"] * self.scale
        row = self._find_row
        baseline = (
            row[1]
            + row[3] / 2
            + chrome_draw.INTER_CAP / chrome_draw.INTER_UNITS * px / 2
        )
        canvas.text(row[0], baseline, FIND_TEXT, 400, px, self.theme.rgb["foreground"])
        if self.popover:
            floating_draw.draw_popover(
                canvas,
                self.theme,
                self.expected,
                self.popover,
                states,
                self.keyboard,
                self.float_fault,
            )
        return np.clip(canvas.array + 0.5, 0, 255).astype(int)
