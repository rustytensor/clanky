"""A simulated clanky-gpui with the Settings window, for G4's gates before the real one exists.

It extends the floating simulation (so the main window and the popovers still work) with a Settings window opened by
Ctrl+, : seven pages of cards (draw_settings.py), controls that change the config file the way the rulebook's
Settings do, a body that scrolls with the wheel, and the `settings` screen of the control record with G4's additions
(RECORD.md). The planted faults (a `fault` switch) are the four of G4's definition:
  card-padding                  every card's text two device px too far right (the design gate)
  setting-not-written           the Animations switch changes on screen and never reaches the config file
  switch-hit-larger             a switch's hit box four device px larger than drawn
  page-control-out-of-tab-order the first field, switch or slider of a page is left out of the Tab order
"""

import json
import sys
import tempfile
from pathlib import Path

import numpy as np

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(SCRIPT_DIR.parent / "floating"))
sys.path.insert(0, str(SCRIPT_DIR.parent / "window-chrome"))
import config_model
import draw_settings
import expected_settings
from chrome_harness import latest_landmarks
from simulated_floating import SimFloatingWindow

SETTINGS_FAULTS = (
    "card-padding",
    "setting-not-written",
    "switch-hit-larger",
    "page-control-out-of-tab-order",
)
WHEEL_NOTCH_LOGICAL_PX = 48  # what one wheel notch scrolls
UNWRITTEN_ROW = (
    "Animations"  # the row the setting-not-written fault leaves out of the config file
)
FIELD_KINDS = ("field", "switch", "slider")


class SimSettingsWindow(SimFloatingWindow):
    def __init__(self, scale, theme="graphite-dark", fault=None, config_path=None):
        self.settings_fault = fault if fault in SETTINGS_FAULTS else None
        self.settings_open, self.page, self.scroll = False, "General", 0
        self.texts, self.select_all, self.error, self.shown = {}, False, "", None
        self.settings_expected = expected_settings.build(scale, theme)
        self.config_path = Path(
            config_path
            or Path(tempfile.mkdtemp(prefix="settings-sim-")) / "config.json"
        )
        self.config = config_model.initial_config()
        config_model.write_config(self.config_path, self.config)
        self.file_seen = dict(self.config)
        super().__init__(scale, theme, None if self.settings_fault else fault)
        self._rebuild()
        self._record()

    # -- the window --------------------------------------------------------------------------------------------

    def _rebuild(self):
        values = {**self.config, **self.texts}
        self.shown = draw_settings.Settings(
            self.settings_expected,
            (self.pw, self.ph),
            self.page,
            values,
            self.scroll,
            self.settings_fault,
        )
        self.scroll = self.shown.scroll

    def _controls(self):
        if self.settings_open:
            return self.shown.controls
        return super()._controls()

    def _hit_box(self, control):
        box = control["box"]
        if (
            self.settings_open
            and control["kind"] == "switch"
            and self.settings_fault == "switch-hit-larger"
        ):
            return [box[0] - 2, box[1] - 2, box[2] + 4, box[3] + 4]
        return super()._hit_box(control)

    def _under_pointer(self):
        if not self.settings_open:
            return super()._under_pointer()
        px, py = self.pointer
        for control in self.shown.controls:
            x, y, w, h = self._hit_box(control)
            if control["visible"] and x <= px < x + w and y <= py < y + h:
                return control
        return None

    # -- the record --------------------------------------------------------------------------------------------

    def _sync_config(self):
        """Live reload: a change to the config file made from outside is taken up."""
        current = config_model.read_config(self.config_path)
        if current != self.file_seen:
            self.config, self.file_seen = dict(current), dict(current)
            self._rebuild()
            self._record()

    def screen(self, name):
        self._sync_config()
        return super().screen(name)

    def _record(self):
        super()._record()
        if self.shown is None:
            return
        states = self._states()
        lines = [self._settings_line(states)]
        marks = self.shown.landmarks() if self.settings_open else []
        banner = {
            "record": "landmark",
            "name": "error-banner",
            "index": 0,
            "box": [0, 0, 0, 0],
            "text": "",
        }
        if self.error:
            panel = self.shown.panel
            banner.update(
                box=[
                    panel[0],
                    panel[1] + panel[3] - self.shown.s * 30,
                    panel[2],
                    round(self.shown.s * 30),
                ],
                text=self.error,
            )
        lines.append(banner)
        seen = {}
        for name, box in marks:
            lines.append(
                {
                    "record": "landmark",
                    "name": name,
                    "index": seen.get(name, 0),
                    "box": box,
                }
            )
            seen[name] = seen.get(name, 0) + 1
        for line in lines:
            key = (
                line["screen"]
                if line["record"] == "screen"
                else f"landmark {line['name']} {line['index']}"
            )
            text = json.dumps(line, sort_keys=True)
            if self._last.get(key) != text:
                self._last[key] = text
                self._records.append(line)
                self.frames += 1

    def _settings_line(self, states):
        controls = []
        if self.settings_open:
            for control in self.shown.controls:
                line = {
                    "id": control["id"],
                    "kind": control["kind"],
                    "label": control["label"],
                    "box": control["box"],
                    "hit": control["box"],
                    "state": states[control["id"]],
                    "enabled": True,
                    "tab": True,
                    "group": control["group"],
                    "axis": control["axis"],
                    "checked": control["checked"],
                    "primary": False,
                    "label_box": control["label_box"],
                    "icon": control["icon"],
                    "visible": control["visible"],
                }
                for extra in ("field_text", "value", "min", "max"):
                    if extra in control:
                        line[extra] = control[extra]
                controls.append(line)
        return {
            "record": "screen",
            "screen": "settings",
            "scale": self.scale,
            "page": self.page,
            "scroll": {"top": self.scroll, "max": self.shown.max_scroll},
            "popover": None,
            "focus": self.focus_id if self.settings_open else None,
            "keyboard": self.keyboard,
            "controls": controls,
        }

    def _activate(self, control, via):
        if not self.settings_open:
            return super()._activate(control, via)
        self._records.append(
            {
                "record": "activate",
                "screen": "settings",
                "id": control["id"],
                "label": control["label"],
                "via": via,
            }
        )
        self._apply(control)

    # -- what a control does -----------------------------------------------------------------------------------

    def _row_of(self, control):
        key = (
            control["group"][4:]
            if control["group"] and control["group"].startswith("row:")
            else None
        )
        return next(
            (r for r in config_model.ROWS if r["key"] == key), None
        ) or config_model.row_for(control["label"])

    def _store(self, row, value):
        self.config[row["key"]] = value
        if not (
            self.settings_fault == "setting-not-written"
            and row["label"] == UNWRITTEN_ROW
        ):
            config_model.write_config(self.config_path, self.config)

    def _apply(self, control):
        kind = control["kind"]
        if control["group"] == "settings-pages":
            self.page, self.scroll, self.focus_id = control["label"], 0, None
        elif kind == "switch":
            row = self._row_of(control)
            self._store(row, not self.config[row["key"]])
        elif kind == "segment":
            row = self._row_of(control)
            self._store(row, config_model.value_for(row, control["label"]))
        elif kind == "slider":
            row = self._row_of(control)
            x, _, w, _ = control["box"]
            share = min(max((self.pointer[0] - x) / max(w - 1, 1), 0.0), 1.0)
            self._store(
                row,
                round(control["min"] + share * (control["max"] - control["min"]), 3),
            )
        self._rebuild()

    def _commit_field(self, control):
        row = self._row_of(control)
        text = self.texts.pop(row["key"], None)
        if text is None:
            return
        try:
            self._store(row, config_model.coerce(row, text))
            self.error = ""
        except ValueError:
            self.error = f"config line 1: {row['key']} is not a number"
        self._rebuild()

    # -- input -------------------------------------------------------------------------------------------------

    def x(self, *args):
        if self.settings_open and args[0] == "click" and args[1] in ("4", "5"):
            step = round(WHEEL_NOTCH_LOGICAL_PX * self.scale)
            self.scroll += step if args[1] == "5" else -step
            self._rebuild()
            self._record()
            return
        super().x(*args)

    def _mouse_down(self):
        if not self.settings_open:
            return super()._mouse_down()
        control = self._under_pointer()
        self.pressed = control["id"] if control else None
        self.keyboard = False
        if control and control["kind"] == "field":
            self.focus_id, self.select_all = control["id"], True
        else:
            if self.focus_id is not None and self._field_in_edit():
                self._commit_field(self._by_id(self.focus_id))
            self.focus_id = None

    def _mouse_up(self):
        if not self.settings_open:
            return super()._mouse_up()
        control = self._under_pointer()
        if (
            self.pressed
            and control
            and control["id"] == self.pressed
            and control["kind"] != "field"
        ):
            self._activate(control, "release")
        self.pressed = None

    def _by_id(self, ident):
        return next((c for c in self.shown.controls if c["id"] == ident), None)

    def _field_in_edit(self):
        control = self._by_id(self.focus_id)
        return bool(
            control
            and control["kind"] == "field"
            and self._row_of(control)["key"] in self.texts
        )

    def _ensure_visible(self):
        control = self._by_id(self.focus_id)
        if not control or control["header"] or control["visible"]:
            return
        body = self.shown.body
        y, h = control["box"][1], control["box"][3]
        self.scroll += (y + h - (body[1] + body[3])) if y >= body[1] else (y - body[1])
        self._rebuild()

    def _order(self):
        """Tab order is reading order of the page as laid out, so a scrolled body keeps its order."""
        if not self.settings_open:
            return super()._order()
        enabled = [c for c in self.shown.controls if c["enabled"]]
        ordered = sorted(
            enabled,
            key=lambda c: (
                round(c["box"][1] + (0 if c["header"] else self.scroll)),
                round(c["box"][0]),
            ),
        )
        order = [c["id"] for c in ordered]
        if self.settings_fault == "page-control-out-of-tab-order":
            skip = next((c["id"] for c in ordered if c["kind"] in FIELD_KINDS), None)
            order = [i for i in order if i != skip]
        return order

    def _key(self, name):
        if name == "ctrl+comma" and not self.popover:
            self.settings_open, self.return_focus, self.focus_id = (
                True,
                self.focus_id,
                None,
            )
            self.page, self.scroll, self.keyboard = "General", 0, False
            self._rebuild()
        elif not self.settings_open:
            super()._key(name)
        elif name == "Escape":
            if self._field_in_edit():
                self.texts.pop(self._row_of(self._by_id(self.focus_id))["key"], None)
                self._rebuild()
            else:
                self.settings_open, self.focus_id = False, self.return_focus
                self.keyboard = self.focus_id is not None
        elif name == "ctrl+a":
            self.select_all = True
        elif name in ("Return", "space") and self._by_id(self.focus_id):
            control = self._by_id(self.focus_id)
            if control["kind"] == "field":
                self._commit_field(control)
            else:
                self._activate(control, "key")
        elif name in ("Tab", "shift+Tab"):
            self._step(self._order(), name == "Tab")
            self._ensure_visible()
            self._rebuild()
        else:
            super()._key(name)
            self._ensure_visible()
            self._rebuild()

    def type(self, text):
        control = self._by_id(self.focus_id) if self.settings_open else None
        if not (control and control["kind"] == "field"):
            return super().type(text) if not self.settings_open else None
        key = self._row_of(control)["key"]
        current = (
            ""
            if self.select_all
            else self.texts.get(key, config_model.display_text(self.config[key]))
        )
        self.texts[key], self.select_all = current + text, False
        self._rebuild()
        self._record()

    # -- what the gates read -----------------------------------------------------------------------------------

    def landmarks(self):
        return latest_landmarks(self._records)

    def shot(self, name):
        image = super().shot(name)
        if not self.settings_open:
            return image
        canvas = draw_settings.ClippedCanvas(self.pw, self.ph, 0)
        canvas.array = image.astype(float)
        draw_settings.draw_page(
            canvas,
            self.theme,
            self.settings_expected,
            self.shown,
            self._states(),
            self.keyboard,
        )
        return np.clip(canvas.array + 0.5, 0, 255).astype(int)
