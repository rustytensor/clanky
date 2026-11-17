"""A simulated clanky-gpui main window that answers the gates' pointer and key input and writes the control record.

The gates are proven on this before the real window exists (G2 builds the writer): it follows the rulebook, so the
good run must pass every check, and each planted fault (a switch in `fault`) must be caught by the check aimed at it.
It offers the same methods as the real window harness: x, key, type, move, away, shot, records, screen, activations,
pane_text, menu_point, close, and the numbers scale, pw, ph.
"""

import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
import draw_chrome_reference as draw  # noqa: E402
from chrome_harness import latest_landmarks  # noqa: E402
import expected as expected_values  # noqa: E402

FIRST_POPOVER_ID = 100
FAULTS = ("hit-larger", "press-fires", "tab-skip", "border-2px", "tab-too-tall")
STATE_ORDER = ("disabled", "pressed", "focus", "hover", "rest")


class SimWindow:
    def __init__(self, scale, theme="graphite-dark", fault=None):
        if fault not in (None, *FAULTS):
            raise ValueError(f"unknown simulated fault {fault}")
        self.scale, self.fault, self.kind = scale, fault, "simulated"
        self.expected = expected_values.build(scale, theme)
        self.layout = draw.Layout(self.expected, fault)
        self.theme = draw.Theme(self.expected)
        self.pw, self.ph = self.layout.width, self.layout.height
        self.wid = "simulated"
        self.pointer = (self.pw - 1, self.ph // 2)
        self.pressed = self.focus_id = self.popover = self.selection_start = (
            self.selection
        ) = None
        self.keyboard = False
        self.typed = ""
        self.pane = ""
        self._records = []
        self._last = {}
        self._emit_landmarks()
        self._record()

    # -- the record --------------------------------------------------------------------------------------------

    def _emit_landmarks(self):
        for line in self.layout.record_lines()[:-1]:
            self._records.append(line)

    def _controls(self):
        """Controls that take input now: the popover's items while it is open, else the main window's."""
        if self.popover:
            return self.popover["items"]
        return self.layout.controls

    def _hit_box(self, control):
        box = control["box"]
        if self.fault == "hit-larger" and "popover" not in control:
            return [box[0] - 2, box[1] - 2, box[2] + 4, box[3] + 4]
        return box

    def _under_pointer(self):
        px, py = self.pointer
        for control in self._controls():
            x, y, w, h = self._hit_box(control)
            if x <= px < x + w and y <= py < y + h:
                return control
        return None

    def _states(self):
        hovered = self._under_pointer()
        states = {}
        for control in self._controls():
            ident = control["id"]
            candidates = {"rest"}
            if not control["enabled"]:
                candidates.add("disabled")
            if self.pressed == ident and hovered and hovered["id"] == ident:
                candidates.add("pressed")
            if self.focus_id == ident and self.keyboard:
                candidates.add("focus")
            if hovered and hovered["id"] == ident:
                candidates.add("hover")
            states[ident] = next(state for state in STATE_ORDER if state in candidates)
        return states

    def _record(self):
        states = self._states()
        main = self.layout.record_lines("main-window", states)[-1]
        main["focus"] = (
            self.focus_id
            if self.focus_id and self.focus_id < FIRST_POPOVER_ID
            else None
        )
        main["keyboard"] = self.keyboard
        menu = {
            "record": "screen",
            "screen": "context-menu",
            "scale": self.scale,
            "focus": None,
            "keyboard": self.keyboard,
            "popover": None,
            "controls": [],
        }
        if self.popover:
            menu["popover"] = {
                "box": self.popover["box"],
                "opener": self.popover["opener"],
                "opener_id": None,
                "opener_pane": "1",
            }
            menu["focus"] = self.focus_id
            menu["controls"] = [
                self._menu_record(item, states) for item in self.popover["items"]
            ]
        for line in (main, menu):
            text = json.dumps(line, sort_keys=True)
            if self._last.get(line["screen"]) != text:
                self._last[line["screen"]] = text
                self._records.append(line)

    def _menu_record(self, item, states):
        return {
            "id": item["id"],
            "kind": "menu-item",
            "label": item["label"],
            "box": item["box"],
            "hit": item["box"],
            "state": states[item["id"]],
            "enabled": True,
            "tab": True,
            "group": "menu",
            "axis": "vertical",
            "checked": None,
            "primary": False,
            "label_box": None,
            "icon": None,
        }

    def _activate(self, control, via):
        screen = "context-menu" if self.popover else "main-window"
        self._records.append(
            {
                "record": "activate",
                "screen": screen,
                "id": control["id"],
                "label": control["label"],
                "via": via,
            }
        )
        if self.popover:
            self.popover = None
            self.focus_id = None

    # -- input -------------------------------------------------------------------------------------------------

    def x(self, *args):
        command = args[0]
        if command == "mousemove":
            self.pointer = (int(args[3]), int(args[4]))
        elif command == "mousedown":
            self._mouse_down()
        elif command == "mouseup":
            self._mouse_up()
        elif command == "click" and args[1] == "3":
            self._open_menu()
        self._record()

    def move(self, x, y):
        self.x("mousemove", "--window", self.wid, str(int(x)), str(int(y)))

    def away(self, point=None):
        self.move(*(point or (int(0.9 * self.pw), int(0.5 * self.ph))))

    def _mouse_down(self):
        control = self._under_pointer()
        if self.popover and control is None:
            self.popover = None
            self.focus_id = None
            return
        if control and control["enabled"]:
            self.pressed = control["id"]
            self.focus_id = control["id"]
            self.keyboard = False
            if self.fault == "press-fires":
                self._activate(control, "release")
            return
        if control is None:
            self.focus_id = None
            self.keyboard = False
        row = self.layout.text_rows["pane-text"]
        if (
            row[0] <= self.pointer[0] < row[0] + row[2]
            and row[1] <= self.pointer[1] < row[1] + row[3]
        ):
            self.selection_start = self.pointer[0]
            self.selection = None

    def _mouse_up(self):
        control = self._under_pointer()
        if (
            self.pressed
            and control
            and control["id"] == self.pressed
            and self.fault != "press-fires"
        ):
            self._activate(control, "release")
            if control["kind"] == "switch":
                control["checked"] = not control["checked"]
        self.pressed = None
        if self.selection_start is not None:
            low, high = sorted((self.selection_start, self.pointer[0]))
            self.selection = (low, high) if high - low > 2 else None
            self.selection_start = None

    def _open_menu(self):
        box, items = draw.popover_layout(
            self.scale, self.expected, self.pointer, (self.pw, self.ph)
        )
        entries = [
            {
                "id": FIRST_POPOVER_ID + i,
                "kind": "menu-item",
                "label": label,
                "box": rect,
                "enabled": True,
                "popover": True,
            }
            for i, (label, rect) in enumerate(zip(draw.MENU_ITEMS, items))
        ]
        self.popover = {
            "box": box,
            "items": entries,
            "opener": [self.pointer[0], self.pointer[1], 0, 0],
        }
        self.focus_id = None
        self.keyboard = False

    def key(self, *keys):
        for name in keys:
            self._key(name)
            self._record()

    def _order(self):
        enabled = [c for c in self._controls() if c["enabled"]]
        ordered = sorted(
            enabled, key=lambda c: (round(c["box"][1]), round(c["box"][0]))
        )
        if self.fault == "tab-skip" and len(ordered) > 2:
            del ordered[2]
        return [c["id"] for c in ordered]

    def _step(self, order, forward):
        if not order:
            return
        if self.focus_id in order:
            index = order.index(self.focus_id)
            self.focus_id = order[(index + (1 if forward else -1)) % len(order)]
        else:
            self.focus_id = order[0] if forward else order[-1]
        self.keyboard = True

    def _key(self, name):
        if name in ("Tab", "shift+Tab"):
            self._step(self._order(), name == "Tab")
        elif name in ("Down", "Up", "Left", "Right"):
            self._arrow(name)
        elif name in ("Return", "space"):
            self._press_key(name)
        elif name == "Escape" and self.popover:
            self.popover = None
            self.focus_id = None

    @staticmethod
    def _group_of(control):
        return control.get("group") or ("menu" if control.get("popover") else None)

    @staticmethod
    def _axis_of(control):
        return control.get("axis") or ("vertical" if control.get("popover") else None)

    def _arrow(self, name):
        axis = "vertical" if name in ("Down", "Up") else "horizontal"
        forward = name in ("Down", "Right")
        controls = [
            c
            for c in self._controls()
            if c["enabled"] and self._group_of(c) and self._axis_of(c) == axis
        ]
        controls.sort(key=lambda c: (round(c["box"][1]), round(c["box"][0])))
        current = next((c for c in controls if c["id"] == self.focus_id), None)
        group = self._group_of(current or (controls[0] if controls else {}))
        order = [c["id"] for c in controls if self._group_of(c) == group]
        if current is not None:
            self._step(order, forward)
        elif order:
            self.focus_id = order[0]
            self.keyboard = True

    def _press_key(self, name):
        control = next(
            (c for c in self._controls() if c["id"] == self.focus_id and c["enabled"]),
            None,
        )
        if control is None:
            if name == "Return" and not self.popover:
                self.pane += "\n" + self.typed
                self.typed = ""
            return
        self._activate(control, "key")
        if control["kind"] == "switch" and name == "space":
            control["checked"] = not control["checked"]

    def type(self, text):
        if not self.popover:
            self.typed += text
            self.pane = self.pane + text

    def pane_text(self, _pane):
        return self.pane

    def menu_point(self):
        return int(0.45 * self.pw), int(0.3 * self.ph)

    # -- what the gates read -----------------------------------------------------------------------------------

    def records(self):
        return list(self._records)

    def screen(self, name):
        found = None
        for line in self._records:
            if line.get("record") == "screen" and line.get("screen") == name:
                found = line
        return found

    def activations(self, name):
        return [
            r
            for r in self._records
            if r.get("record") == "activate" and r.get("screen") == name
        ]

    def landmarks(self):
        return latest_landmarks(self._records)

    def shot(self, _name):
        states = self._states()
        popover = None
        if self.popover:
            popover = {"box": self.popover["box"], "items": self.popover["items"]}
        image = draw.draw_window(
            self.layout,
            self.theme,
            states,
            self.keyboard,
            popover,
            self.selection,
            self.fault,
        )
        return image.astype(int)

    def close(self):
        return None
