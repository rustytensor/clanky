#!/usr/bin/env python3
"""Draws the main window as it should look, from expected.py's values, and lays out its control record.

Usage: python3 draw_chrome_reference.py --scale S --theme T --out-dir DIR

writes DIR/record-good-S.jsonl and DIR/main-window-good-S.png: a synthetic window (title bar with a tools toggle, two
tabs and a new-tab button, the tools panel with its list rows, switches and segmented switcher, two panes, a row of
buttons, a switch, a field, a chip, a status line) in its rest state. The panel shows the controls of all three of its
tools at once, so that every kind is on one screen. The simulated window
(simulated_window.py) uses the same drawing for every state, and the gates are proven on it before the real window
exists. Nothing here reads the program under test.
"""

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
import expected as expected_values  # noqa: E402

REPO_ROOT = SCRIPT_DIR.parents[1]
FONTS = {
    400: REPO_ROOT / "assets/fonts/Inter-Regular.ttf",
    500: REPO_ROOT / "assets/fonts/Inter-Medium.ttf",
}
SUPERSAMPLE = 8
LOGICAL_W, LOGICAL_H = 1200, 800
INTER_UNITS, INTER_CAP = 2048, 1490
MENU_WIDTH, MENU_PADDING, MENU_ITEMS = 160, 4, ("Copy", "Paste", "Select all")
SELECTION_ROW_TEXT = "echo g1-ok"
PANEL_WIDTH = 236  # logical px, the tools panel at the window's left
SEGMENT_WIDTH, SEGMENT_GAP, TRAY_PADDING = 70, 2, 2
TOOLS = ("Files", "Recent", "Agents")
FIRST_SEGMENT_ID, FIRST_ROW_ID, FIRST_SWITCH_ID, TOOLS_TOGGLE_ID = 50, 1000, 4000, 2
PANEL_ROWS = ("src", "Cargo.toml", "README.md")
RECENT_ROWS = ("Split right", "New tab")
AGENT_ROWS = ("Fake Tool", "Codex")
FIRST_RECENT_ID, FIRST_AGENT_ID = 2000, 3000
FIRST_AUTO_ID = 10
ROW_KINDS = (
    "menu-item",
    "list-row",
    "palette-row",
)  # a focused row is lifted as a hovered one


def round_half_up(value):
    return math.floor(value + 0.5)


def hex_rgb(text):
    return np.array([int(text[i : i + 2], 16) for i in (1, 3, 5)], dtype=float)


class Theme:
    """Colours of one theme as arrays, from expected.py's JSON and the rulebook's border and input rules."""

    def __init__(self, expected):
        colours = expected["colours"]
        tokens = expected_values.load_tokens()["themes"][expected["theme"]]
        self.rgb = {
            role: hex_rgb(entry["hex"])
            for role, entry in colours.items()
            if "hex" in entry
        }
        self.border = (hex_rgb(tokens["border"]["over"]), tokens["border"]["alpha"])
        self.input = (hex_rgb(tokens["input"]["over"]), tokens["input"]["alpha"])
        looks = expected["looks"]["switch"]
        self.switch_on_track = self.rgb[looks["on_track"]["role"]]
        self.switch_off_track = self.rgb[looks["off_track"]["role"]]
        self.knob_off = self.rgb[looks["off_knob"]["role"]]
        self.knob_on = self.rgb[looks["on_knob"]["role"]]
        active_tab = expected_values.load_tokens()["active_tab"]
        self.tab_fill_alpha = active_tab["fill_foreground_alpha"]["value"]
        self.tab_ring_alpha = active_tab["inner_ring_foreground_alpha"]["value"]


class Canvas:
    def __init__(self, width, height, colour):
        self.array = np.empty((height, width, 3), dtype=float)
        self.array[:] = colour
        self.width, self.height = width, height

    def composite(self, left, top, mask, colour, alpha):
        """Blends colour into the rectangle at (left, top) through a 0 to 1 mask."""
        h, w = mask.shape
        region = self.array[top : top + h, left : left + w]
        weight = (mask * alpha)[..., None]
        region[:] = region * (1 - weight) + np.asarray(colour) * weight

    def rect(self, box, colour, alpha=1.0):
        x, y, w, h = box
        self.composite(x, y, np.ones((h, w)), colour, alpha)

    def rounded(self, box, radius, colour, alpha=1.0, outline=None):
        """A rounded rectangle, or with outline=width only its border of that width inside the box."""
        x, y, w, h = box
        mask = rounded_mask(w, h, radius)
        if outline:
            inner = np.zeros((h, w))
            inner[outline : h - outline, outline : w - outline] = rounded_mask(
                w - 2 * outline, h - 2 * outline, max(radius - outline, 0)
            )
            mask = np.clip(mask - inner, 0, 1)
        self.composite(x, y, mask, colour, alpha)

    def ring_outside(self, box, radius, width, colour, alpha):
        """A ring of the given width just outside the box."""
        x, y, w, h = box
        outer = (x - width, y - width, w + 2 * width, h + 2 * width)
        mask = rounded_mask(outer[2], outer[3], radius + width)
        mask[width : width + h, width : width + w] = np.minimum(
            mask[width : width + h, width : width + w], 1 - rounded_mask(w, h, radius)
        )
        self.composite(outer[0], outer[1], mask, colour, alpha)

    def text(self, left, baseline, text, weight, size_px, colour, alpha=1.0):
        """Text drawn large and reduced, so it carries no hinting; left and baseline are device pixels, fractions allowed."""
        font = ImageFont.truetype(str(FONTS[weight]), round(size_px * SUPERSAMPLE))
        advance = font.getlength(text) / SUPERSAMPLE
        x0, y0 = math.floor(left) - 2, math.floor(baseline - size_px) - 2
        width, height = math.ceil(advance) + 6, math.ceil(size_px * 1.6) + 4
        canvas = Image.new("L", (width * SUPERSAMPLE, height * SUPERSAMPLE), 0)
        ImageDraw.Draw(canvas).text(
            ((left - x0) * SUPERSAMPLE, (baseline - y0) * SUPERSAMPLE),
            text,
            font=font,
            fill=255,
            anchor="ls",
        )
        mask = np.asarray(canvas.resize((width, height), Image.BOX), dtype=float) / 255
        self.composite(x0, y0, mask, colour, alpha)
        return advance

    def plus(self, box, thickness, colour, alpha):
        """A plus sign centred in the box, as an icon stand-in with symmetric ink."""
        x, y, w, h = box
        mask = Image.new("L", (w * SUPERSAMPLE, h * SUPERSAMPLE), 0)
        draw = ImageDraw.Draw(mask)
        cx, cy, half, t = (
            w * SUPERSAMPLE / 2,
            h * SUPERSAMPLE / 2,
            w * SUPERSAMPLE * 0.32,
            thickness * SUPERSAMPLE / 2,
        )
        draw.rectangle((cx - half, cy - t, cx + half, cy + t), fill=255)
        draw.rectangle((cx - t, cy - half, cx + t, cy + half), fill=255)
        self.composite(
            x,
            y,
            np.asarray(mask.resize((w, h), Image.BOX), dtype=float) / 255,
            colour,
            alpha,
        )


def rounded_mask(width, height, radius):
    mask = Image.new("L", (width * SUPERSAMPLE, height * SUPERSAMPLE), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, width * SUPERSAMPLE - 1, height * SUPERSAMPLE - 1),
        radius=radius * SUPERSAMPLE,
        fill=255,
    )
    return np.asarray(mask.resize((width, height), Image.BOX), dtype=float) / 255


def blend(base, over, alpha):
    return base + (over - base) * alpha


def box_of(scale, left, top, width, height):
    """A box from logical numbers: edges rounded to whole device pixels, as painting rounds them."""
    x0, y0 = round_half_up(left * scale), round_half_up(top * scale)
    x1, y1 = (
        round_half_up((left + width) * scale),
        round_half_up((top + height) * scale),
    )
    return [x0, y0, x1 - x0, y1 - y0]


def text_width(label, weight, px):
    font = ImageFont.truetype(str(FONTS[weight]), round(px * SUPERSAMPLE))
    return font.getlength(label) / SUPERSAMPLE


class Layout:
    """The synthetic main window's controls and landmarks, for one scale and the expected values."""

    def __init__(self, expected, fault=None):
        self.expected, self.fault = expected, fault
        self.scale = expected["scale"]
        self.width, self.height = (
            round(LOGICAL_W * self.scale),
            round(LOGICAL_H * self.scale),
        )
        self.all_controls = []
        self.view = TOOLS[
            0
        ]  # the tool the panel shows; its controls are on screen, the others' are not
        self.landmarks = []
        self._next_id = FIRST_AUTO_ID
        self._build()

    @property
    def controls(self):
        """The controls on screen now: every one that belongs to no tool, and the shown tool's."""
        return [c for c in self.all_controls if c["view"] in (None, self.view)]

    def kind_logical(self, kind, key="height"):
        return self.expected["controls"][kind][key]["logical"]

    def add(self, kind, label, left, top, width, **extra):
        s = self.scale
        height = self.kind_logical(kind)
        if kind == "tab" and self.fault == "tab-too-tall":
            height += 2
        ident = extra.pop("ident", None)
        if ident is None:
            ident = self._next_id
            self._next_id += 1
        control = {
            "id": ident,
            "kind": kind,
            "label": label,
            "box": box_of(s, left, top, width, height),
            "enabled": extra.pop("enabled", True),
            "group": extra.pop("group", None),
            "axis": extra.pop("axis", None),
            "checked": extra.pop("checked", None),
            "primary": False,
            "label_box": None,
            "icon": None,
            "view": extra.pop("view", None),
        }
        control.update(extra)
        self.all_controls.append(control)
        return control

    def _build(self):
        s, e = self.scale, self.expected
        chrome = e["chrome"]
        bar_h, status_h = (
            chrome["title_bar_height"]["logical"],
            chrome["status_height"]["logical"],
        )
        margin, gap = chrome["pane_margin"]["logical"], chrome["pane_gap"]["logical"]
        tab_top = (bar_h - self.kind_logical("tab")) / 2
        self.add(
            "icon-button",
            "Tools panel",
            12,
            (bar_h - 28) / 2,
            28,
            ident=TOOLS_TOGGLE_ID,
        )
        tab = self.add(
            "tab",
            "HIH",
            48,
            tab_top,
            120,
            checked=True,
            group="tabs",
            axis="horizontal",
        )
        tab["icon_logical"] = True
        self.add(
            "tab",
            "Build",
            172,
            tab_top,
            120,
            checked=False,
            group="tabs",
            axis="horizontal",
        )
        self.add("icon-button", "", 296, (bar_h - 28) / 2, 28)
        top = bar_h + gap
        pane_h = LOGICAL_H - status_h - margin - top
        self.panel = box_of(s, margin, top, PANEL_WIDTH, pane_h)
        left = margin + PANEL_WIDTH + gap
        self.content_left = left
        pane_w = (LOGICAL_W - left - margin - gap) / 2
        self.panes = [
            box_of(s, left, top, pane_w, pane_h),
            box_of(s, left + pane_w + gap, top, pane_w, pane_h),
        ]
        self._build_panel(margin, top, pane_h)
        for index, label in enumerate(("Open", "Save", "Share", "Delete")):
            self.add(
                "button",
                label,
                left + 16 + index * 104,
                64,
                96,
                enabled=label != "Delete",
            )
        self.add("switch", "", left + 16, 112, 40, checked=True)
        self.add("field", "Command", left + 16, 152, 200)
        self.add("chip", "Shell", left + 16, 200, 72)
        self.landmarks = self._landmarks(bar_h, status_h)
        for control in self.all_controls:
            self._place_content(control)

    def _build_panel(self, left, top, height):
        """The tools panel: list rows, the Agents switches and the segmented switcher at the foot, in a tray."""
        row_h = self.kind_logical("list-row")
        for index, label in enumerate(PANEL_ROWS):
            self.add(
                "list-row",
                label,
                left + 8,
                top + 40 + index * (row_h + 2),
                PANEL_WIDTH - 16,
                ident=FIRST_ROW_ID + index,
                group="files",
                axis="vertical",
                view="Files",
            )
        for index, label in enumerate(RECENT_ROWS):
            self.add(
                "list-row",
                label,
                left + 8,
                top + 40 + index * (row_h + 2),
                PANEL_WIDTH - 16,
                ident=FIRST_RECENT_ID + index,
                group="recent",
                axis="vertical",
                view="Recent",
            )
        for index, label in enumerate(AGENT_ROWS):
            self.add(
                "list-row",
                label,
                left + 8,
                top + 40 + index * (row_h + 2),
                PANEL_WIDTH - 16,
                ident=FIRST_AGENT_ID + index,
                group="agents",
                axis="vertical",
                view="Agents",
            )
        for index, on in enumerate((True, False)):
            self.add(
                "switch",
                "",
                left + PANEL_WIDTH - 8 - 40,
                top + 160 + index * 32,
                40,
                ident=FIRST_SWITCH_ID + index,
                checked=on,
                view="Agents",
            )
        seg_h = self.kind_logical("segment")
        tray_w = 3 * SEGMENT_WIDTH + 2 * SEGMENT_GAP + 2 * TRAY_PADDING
        tray_top = top + height - 8 - seg_h - 2 * TRAY_PADDING
        self.tray = box_of(
            self.scale, left + 8, tray_top, tray_w, seg_h + 2 * TRAY_PADDING
        )
        for index, label in enumerate(TOOLS):
            self.add(
                "segment",
                label,
                left + 8 + TRAY_PADDING + index * (SEGMENT_WIDTH + SEGMENT_GAP),
                tray_top + TRAY_PADDING,
                SEGMENT_WIDTH,
                ident=FIRST_SEGMENT_ID + index,
                group="tools",
                axis="horizontal",
                checked=index == 0,
            )

    def _landmarks(self, bar_h, status_h):
        s = self.scale
        landmarks = [
            ("title-bar", [0, 0, self.width, round_half_up(bar_h * s)]),
            (
                "status-line",
                [
                    0,
                    self.height - round_half_up(status_h * s),
                    self.width,
                    round_half_up(status_h * s),
                ],
            ),
        ]
        landmarks += [("pane", box) for box in self.panes]
        row = self.expected["cap_heights"]["text_ui"]["size_px"]
        line = round_half_up(row * 1.2 * s)
        self.text_rows = {
            "pane-text": [
                round_half_up((self.content_left + 16) * s),
                round_half_up(240 * s),
                round_half_up(300 * s),
                line,
            ],
            "meta-text": [
                round_half_up((self.content_left + 16) * s),
                round_half_up(280 * s),
                round_half_up(60 * s),
                line,
            ],
        }
        landmarks += list(self.text_rows.items())
        px = self.expected["cap_heights"]["text_status"]["size_px"] * s
        digit_w = round_half_up(text_width("1", 400, px)) + 2
        landmarks.append(
            (
                "status-count",
                [
                    round_half_up(12 * s),
                    self.height - round_half_up(status_h * s) + 1,
                    digit_w,
                    round_half_up(status_h * s) - 1,
                ],
            )
        )
        return landmarks

    def _place_content(self, control):
        """Label and icon boxes, centred as the rulebook asks: cap middle on the box middle, icon in the box middle."""
        s = self.scale
        kind, (x, y, w, h) = control["kind"], control["box"]
        spec = self.expected["controls"][kind]
        icon = round_half_up(spec["icon"]["logical"] * s)
        gap = round_half_up(6 * s)
        text = spec.get("text_px")
        weight = 500 if kind == "tab" and control["checked"] else 400
        label_w = (
            text_width(control["label"], weight, text * s)
            if control["label"] and text
            else 0
        )
        has_icon = kind == "icon-button" or control.pop("icon_logical", False)
        both = (
            (icon if has_icon else 0) + (gap if has_icon and label_w else 0) + label_w
        )
        left = x + (w - both) / 2
        if kind == "list-row":
            left = x + round_half_up(8 * s)
        if has_icon:
            control["icon"] = [
                round_half_up(left),
                y + round_half_up((h - icon) / 2),
                icon,
                icon,
            ]
            left += icon + gap
        if label_w:
            control["label_box"] = [round_half_up(left), y, round_half_up(label_w), h]
            control["label_left"] = left
            control["label_weight"] = weight

    def record_lines(self, screen="main-window", state=None):
        controls = []
        for control in self.controls:
            hit = list(control["box"])
            if self.fault == "hit-larger":
                hit = [hit[0] - 2, hit[1] - 2, hit[2] + 4, hit[3] + 4]
            controls.append(
                {
                    "id": control["id"],
                    "kind": control["kind"],
                    "label": control["label"],
                    "box": control["box"],
                    "hit": hit,
                    "state": (state or {}).get(
                        control["id"], "disabled" if not control["enabled"] else "rest"
                    ),
                    "enabled": control["enabled"],
                    "tab": control["enabled"],
                    "group": control["group"],
                    "axis": control["axis"],
                    "checked": control["checked"],
                    "primary": False,
                    "label_box": control["label_box"],
                    "icon": control["icon"],
                }
            )
        seen, lines = {}, []
        for name, box in self.landmarks:
            lines.append(
                {
                    "record": "landmark",
                    "name": name,
                    "index": seen.get(name, 0),
                    "box": box,
                }
            )
            seen[name] = seen.get(name, 0) + 1
        lines.append(
            {
                "record": "screen",
                "screen": screen,
                "scale": self.scale,
                "focus": None,
                "keyboard": False,
                "popover": None,
                "controls": controls,
            }
        )
        return lines


def draw_control(
    canvas, theme, expected, control, state, keyboard, container, fault=None
):
    """One control in its state: ring, fill, border, then content, all at half strength when disabled."""
    s = expected["scale"]
    kind, box = control["kind"], control["box"]
    spec = expected["controls"][kind]
    radius = spec["radius"]["device"]
    fade = 0.5 if state == "disabled" else 1.0
    hair = 1 if fault != "border-2px" else 2
    ring = theme.rgb["ring"]
    if state == "focus" and keyboard:
        width = round_half_up(2 * s)
        canvas.ring_outside(
            box, radius, width, ring, expected["states"]["focus"]["ring_alpha"]
        )
    fill = None
    if kind == "switch":
        fill = theme.switch_on_track if control["checked"] else theme.switch_off_track
    elif kind == "slider":
        fill = theme.rgb["accent"]  # a track of one look in every state, as the switch
    elif kind == "field":
        fill = None
    elif kind == "segment" and control["checked"]:
        # A selected segment keeps its own fill under the pointer and in focus; only a press changes it.
        fill = theme.rgb["pressed" if state == "pressed" else "background"]
        if state == "hover" and fault == "selected-segment-hover":
            fill = theme.rgb["accent"]
    elif state == "pressed":
        fill = theme.rgb["pressed"]
    elif state == "hover" or (state == "focus" and kind in ROW_KINDS):
        fill = theme.rgb["accent"]
    elif kind == "tab" and control["checked"]:
        fill = blend(container, theme.rgb["foreground"], theme.tab_fill_alpha)
    if fill is not None:
        canvas.rounded(box, radius, fill, fade)
    if state == "focus" and keyboard:
        canvas.rounded(box, radius, ring, 1.0, outline=hair)
    elif kind == "tab" and control["checked"]:
        canvas.rounded(
            box,
            radius,
            theme.rgb["foreground"],
            theme.tab_ring_alpha * fade,
            outline=hair,
        )
    elif kind in (
        "button",
        "button-small",
        "button-large",
        "chip",
        "field",
        "switch",
        "slider",
    ):
        canvas.rounded(box, radius, theme.input[0], theme.input[1] * fade, outline=hair)
    ink = theme.rgb["foreground"]
    x, y, w, h = box
    if kind == "switch":
        inset = round_half_up(2 * s)
        knob = h - 2 * inset
        kx = x + w - knob - inset if control["checked"] else x + inset
        canvas.rounded(
            [kx, y + inset, knob, knob],
            knob / 2,
            theme.knob_on if control["checked"] else theme.knob_off,
            fade,
        )
        return
    if kind == "slider":
        inset = round_half_up(2 * s)
        knob = h - 2 * inset
        share = (control["value"] - control["min"]) / (control["max"] - control["min"])
        kx = x + inset + round(share * (w - 2 * inset - knob))
        canvas.rounded(
            [kx, y + inset, knob, knob], knob / 2, theme.rgb["foreground"], fade
        )
        return
    if control["icon"]:
        canvas.plus(control["icon"], 1.75 * s, ink, fade)
    if control["label_box"]:
        px = spec["text_px"] * s
        baseline = y + h / 2 + INTER_CAP / INTER_UNITS * px / 2
        canvas.text(
            control["label_left"],
            baseline,
            control["label"],
            control["label_weight"],
            px,
            ink,
            fade,
        )


def draw_window(
    layout, theme, states=None, keyboard=False, popover=None, selection=None, fault=None
):
    """The whole window in the given states: {control id: state}, an open popover, a selection (x0, x1 in device px)."""
    e, s = layout.expected, layout.scale
    states = states or {}
    frame, background = theme.rgb["frame"], theme.rgb["background"]
    canvas = Canvas(layout.width, layout.height, frame)
    for box in layout.panes:
        canvas.rounded(box, e["chrome"]["pane_radius"]["device"], background)
        canvas.rounded(
            box,
            e["chrome"]["pane_radius"]["device"],
            theme.border[0],
            theme.border[1],
            outline=1,
        )
    panel = layout.panel
    canvas.rounded(panel, e["chrome"]["pane_radius"]["device"], background)
    canvas.rounded(
        panel,
        e["chrome"]["pane_radius"]["device"],
        theme.border[0],
        theme.border[1],
        outline=1,
    )
    canvas.rounded(
        layout.tray, e["controls"]["button"]["radius"]["device"], theme.rgb["card"]
    )
    status = dict(layout.landmarks)["status-line"]
    canvas.rect([0, status[1], layout.width, 1], theme.border[0], theme.border[1])
    ink, muted = theme.rgb["foreground"], theme.rgb["muted_foreground"]
    row = layout.text_rows["pane-text"]
    font_px = e["cap_heights"]["text_ui"]["size_px"] * s
    if selection:
        canvas.rect(
            [selection[0], row[1], selection[1] - selection[0], row[3]],
            theme.rgb["selection"],
        )
    cap = INTER_CAP / INTER_UNITS
    canvas.text(
        row[0],
        row[1] + row[3] / 2 + cap * font_px / 2,
        SELECTION_ROW_TEXT,
        400,
        font_px,
        ink,
    )
    meta = layout.text_rows["meta-text"]
    meta_px = e["cap_heights"]["text_meta"]["size_px"] * s
    canvas.text(
        meta[0], meta[1] + meta[3] / 2 + cap * meta_px / 2, "HIH", 400, meta_px, muted
    )
    status_px = e["cap_heights"]["text_status"]["size_px"] * s
    count = dict(layout.landmarks)["status-count"]
    canvas.text(
        count[0],
        status[1] + 1 + (status[3] - 1) / 2 + cap * status_px / 2,
        "1",
        400,
        status_px,
        muted,
    )
    for control in layout.controls:
        x, y = control["box"][:2]
        container = canvas.array[
            y + control["box"][3] // 2, max(x - 1 - round_half_up(2 * s) - 2, 0)
        ].copy()
        draw_control(
            canvas,
            theme,
            e,
            control,
            states.get(control["id"], "disabled" if not control["enabled"] else "rest"),
            keyboard,
            container,
            fault,
        )
    if popover:
        draw_popover(canvas, theme, e, popover, states, keyboard)
    return np.clip(canvas.array + 0.5, 0, 255).astype(np.uint8)


def popover_layout(scale, expected, point, window):
    """The context menu's box and its item boxes, kept inside the window, flipped when there is no room."""
    s = scale
    item_h = round_half_up(expected["controls"]["menu-item"]["height"]["logical"] * s)
    pad = round_half_up(MENU_PADDING * s)
    width, height = round_half_up(MENU_WIDTH * s), len(MENU_ITEMS) * item_h + 2 * pad
    x, y = point
    if x + width > window[0]:
        x = point[0] - width - 2
    if y + height > window[1]:
        y = point[1] - height - 2
    box = [x + 2, y + 2, width, height]
    items = [
        [box[0] + pad, box[1] + pad + i * item_h, width - 2 * pad, item_h]
        for i in range(len(MENU_ITEMS))
    ]
    return box, items


def draw_popover(canvas, theme, expected, popover, states, keyboard):
    s = expected["scale"]
    card = theme.rgb["card"]
    canvas.rounded(popover["box"], round_half_up(10 * s), card)
    canvas.rounded(
        popover["box"],
        round_half_up(10 * s),
        theme.border[0],
        theme.border[1],
        outline=1,
    )
    for control in popover["items"]:
        state = states.get(control["id"], "rest")
        spec = expected["controls"]["menu-item"]
        box = control["box"]
        if state in ("hover", "pressed", "focus"):
            fill = theme.rgb["pressed"] if state == "pressed" else theme.rgb["accent"]
            canvas.rounded(box, spec["radius"]["device"], fill)
        if state == "focus" and keyboard:
            width = round_half_up(2 * s)
            canvas.ring_outside(
                box,
                spec["radius"]["device"],
                width,
                theme.rgb["ring"],
                expected["states"]["focus"]["ring_alpha"],
            )
            canvas.rounded(
                box, spec["radius"]["device"], theme.rgb["ring"], 1.0, outline=1
            )
        px = spec["text_px"] * s
        baseline = box[1] + box[3] / 2 + INTER_CAP / INTER_UNITS * px / 2
        canvas.text(
            box[0] + round_half_up(8 * s),
            baseline,
            control["label"],
            400,
            px,
            theme.rgb["foreground"],
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--scale", type=float, required=True)
    parser.add_argument("--theme", default="graphite-dark")
    parser.add_argument("--out-dir", type=Path, required=True)
    arguments = parser.parse_args()
    expected = expected_values.build(arguments.scale, arguments.theme)
    layout = Layout(expected)
    theme = Theme(expected)
    arguments.out_dir.mkdir(parents=True, exist_ok=True)
    name = f"{arguments.scale:g}"
    (arguments.out_dir / f"record-good-{name}.jsonl").write_text(
        "\n".join(json.dumps(line, sort_keys=True) for line in layout.record_lines())
        + "\n",
        encoding="utf-8",
    )
    Image.fromarray(draw_window(layout, theme)).save(
        arguments.out_dir / f"main-window-good-{name}.png"
    )


if __name__ == "__main__":
    main()
