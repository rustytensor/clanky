"""Lays out and draws G4's Settings window from expected_settings.py's values: the panel, its header with the section
switcher, and a page of cards, one per setting.

Used by simulated_settings.py to give the gates a window to run on before the real one exists, and by gate-settings.sh
to write the committed fixtures. The layout is a stand-in with the right sizes: every size, radius, padding and gap
comes from the expected values, the positions are this file's choice. Nothing here reads the program under test.
"""

import json
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(SCRIPT_DIR.parent / "window-chrome"))
sys.path.insert(0, str(SCRIPT_DIR.parent / "floating"))
import config_model
import draw_chrome_reference as chrome_draw
import draw_floating

round_half_up = chrome_draw.round_half_up
ROWS = json.loads((SCRIPT_DIR / "rows.json").read_text(encoding="utf-8"))
PAGES = ROWS["pages"]
TITLE_LINE, DESCRIPTION_LINE, LABEL_LINE = (
    16,
    14,
    16,
)  # logical px: the line boxes of the card's texts and a label
FIRST_PAGE_ID, FIRST_ROW_ID, FIRST_OPTION_ID = 10, 200, 400
FIELD_WIDTH, SLIDER_WIDTH, SWITCH_WIDTH = 120, 160, 40
SEGMENT_PAD, TRAY_PAD, SEGMENT_GAP = 12, 2, 2
GROUPS = {  # the group label above a row, where one starts a group
    "Post-AI mode": "Startup",
    "Tabs": "Layout",
    "Follow the system": "Theme",
    "Font size": "Text",
    "Opacity": "Window",
    "Cursor blink": "Cursor",
    "Clipboard writes": "Clipboard",
    "Dictionary": "Compose",
    "New agent panes": "New panes",
    "Save history": "History",
}
CARD_SHIFT_PX = (
    2  # the planted fault: every card's text this far to the right (device px)
)
FONT_ASCENT = {}


def ascent(weight, px):
    """The font's ascender in px: the distance from a line box's top down to its baseline."""
    if weight not in FONT_ASCENT:
        FONT_ASCENT[weight] = (
            TTFont(str(chrome_draw.FONTS[weight]))["hhea"].ascent / 2048
        )
    return FONT_ASCENT[weight] * px


def option_width(label, px):
    return round_half_up(
        chrome_draw.text_width(label, 400, px) + 2 * SEGMENT_PAD * px / 11.5
    )


class Placer:
    """Gives chrome_draw.Layout's content placing to controls that are not in a Layout."""

    def __init__(self, expected):
        self.scale, self.expected = expected["scale"], expected

    _place_content = chrome_draw.Layout._place_content


def device(expected, name, key="device_rounded"):
    return expected["settings"][name][key]


class Settings:
    """One Settings page at one scroll position, in device pixels: boxes, controls, landmarks, scrolling."""

    def __init__(self, expected, window, page, values, scroll=0, fault=None):
        self.e, self.s, self.page, self.values, self.fault = (
            expected,
            expected["scale"],
            page,
            values,
            fault,
        )
        self.window = window
        self.controls, self.cards, self.labels = [], [], []
        self._build_frame()
        self.content = self._build_page()
        self.max_scroll = max(0, self.content - self.body[3])
        self.scroll = min(max(scroll, 0), self.max_scroll)
        self._apply_scroll()

    # -- frame -------------------------------------------------------------------------------------------------

    def _build_frame(self):
        s, e = self.s, self.e
        width = device(e, "panel_width")
        height = round_half_up(
            min(
                self.window[1] / s - 2 * e["settings"]["panel_margin"]["logical"],
                e["settings"]["panel_height_max"]["logical"],
            )
            * s
        )
        x, y = (self.window[0] - width) // 2, (self.window[1] - height) // 2
        self.panel = [x, y, width, height]
        header = device(e, "header_height")
        self.header = [x, y, width, header]
        self.body = [x, y + header, width, height - header]
        self.side = device(e, "padding_side")
        self.column = [x + self.side, 0, width - 2 * self.side, 0]
        self._build_pill()

    def _build_pill(self):
        s, e = self.s, self.e
        pill_h, seg_h = (
            device(e, "tab_pill_height"),
            e["controls"]["segment"]["height"]["device_rounded"],
        )
        px = e["controls"]["segment"]["text_px"] * s
        pad = round_half_up(TRAY_PAD * s)
        left = self.column[0]
        top = self.header[1] + (self.header[3] - pill_h) // 2
        widths = [option_width(name, px) for name in PAGES]
        total = (
            sum(widths) + (len(PAGES) - 1) * round_half_up(SEGMENT_GAP * s) + 2 * pad
        )
        self.pill = [left, top, total, pill_h]
        at = left + pad
        for index, (name, w) in enumerate(zip(PAGES, widths)):
            self._add(
                "segment",
                name,
                [at, top + (pill_h - seg_h) // 2, w, seg_h],
                FIRST_PAGE_ID + index,
                group="settings-pages",
                axis="horizontal",
                checked=name == self.page,
                header=True,
            )
            at += w + round_half_up(SEGMENT_GAP * s)

    # -- page --------------------------------------------------------------------------------------------------

    def _add(self, kind, label, box, ident, header=False, **extra):
        control = {
            "id": ident,
            "kind": kind,
            "label": label,
            "box": box,
            "enabled": True,
            "group": None,
            "axis": None,
            "checked": None,
            "primary": False,
            "label_box": None,
            "icon": None,
            "header": header,
        }
        control.update(extra)
        Placer(self.e)._place_content(control)
        if kind in ("field", "slider"):
            control["label_box"] = (
                None  # the label is the row's title; the field's text is drawn on its own
            )
        self.controls.append(control)
        return control

    def _build_page(self):
        """Cards of the page's rows below the header; returns the page's height in device px."""
        s, e = self.s, self.e
        y = self.body[1] + device(e, "padding_top")
        gap, group_gap = device(e, "card_gap"), device(e, "group_gap")
        card_h = self._card_height()
        rows = [(i, r) for i, r in enumerate(ROWS["rows"]) if r["page"] == self.page]
        first = True
        for index, row in rows:
            label = GROUPS.get(row["label"])
            if label:
                y += 0 if first else group_gap
                self.labels.append(
                    {
                        "text": label,
                        "box": [
                            self.column[0],
                            y,
                            self.column[2],
                            round_half_up(LABEL_LINE * s),
                        ],
                    }
                )
                y += round_half_up(LABEL_LINE * s) + gap
            first = False
            lines = self._description_lines(row)
            height = card_h + (len(lines) - 1) * round_half_up(DESCRIPTION_LINE * s)
            self._card(index, row, y, height, lines)
            y += height + gap
        if self.page == "Keys":
            self.keys_rows = True
        return y - gap + device(e, "padding_bottom") - self.body[1]

    def _card_height(self):
        s, e = self.s, self.e
        pad = device(e, "card_padding_y")
        lines = (
            round_half_up(TITLE_LINE * s)
            + device(e, "title_gap")
            + round_half_up(DESCRIPTION_LINE * s)
        )
        return 2 * (1 + pad) + lines

    def _control_width(self, row):
        """The device px the row's control takes at the card's right, a segmented row's tray included."""
        s, e = self.s, self.e
        fixed = {"switch": SWITCH_WIDTH, "field": FIELD_WIDTH, "slider": SLIDER_WIDTH}
        if row["control"] in fixed:
            return round_half_up(fixed[row["control"]] * s)
        px = e["controls"]["segment"]["text_px"] * s
        widths = [option_width(name, px) for name in row["options"]]
        gap, pad = round_half_up(SEGMENT_GAP * s), round_half_up(TRAY_PAD * s)
        return sum(widths) + (len(widths) - 1) * gap + 2 * pad

    def _description_lines(self, row):
        """The description broken into lines at spaces, each fitting between the card's text edge and the gap
        before its control: a description too long for one line wraps and makes its card taller, never runs
        under the control."""
        s, e = self.s, self.e
        pad_x = device(e, "card_padding_x")
        left = self.column[0] + 1 + pad_x
        right = self.column[0] + self.column[2] - 1 - pad_x
        room = right - self._control_width(row) - device(e, "control_gap") - left
        px = e["cap_heights"]["text_desc"]["size_px"] * s
        lines = []
        for word in row["description"].split():
            if lines and chrome_draw.text_width(f"{lines[-1]} {word}", 400, px) <= room:
                lines[-1] = f"{lines[-1]} {word}"
            else:
                lines.append(word)
        return lines or [""]

    def _card(self, index, row, y, height, lines):
        s, e = self.s, self.e
        box = [self.column[0], y, self.column[2], height]
        pad_x = device(e, "card_padding_x")
        self.cards.append(
            {
                "row": row,
                "index": index,
                "box": box,
                "lines": lines,
                "text_left": box[0]
                + 1
                + pad_x
                + (CARD_SHIFT_PX if self.fault == "card-padding" else 0),
            }
        )
        right = box[0] + box[2] - 1 - pad_x
        mid = y + height // 2
        value = self.values.get(row["key"], row["initial"])
        kind = row["control"]
        if kind == "switch":
            h = e["controls"]["switch"]["height"]["device_rounded"]
            w = round_half_up(SWITCH_WIDTH * s)
            self._add(
                "switch",
                row["label"],
                [right - w, mid - h // 2, w, h],
                FIRST_ROW_ID + index,
                checked=bool(value),
            )
        elif kind == "field":
            h = e["controls"]["field"]["height"]["device_rounded"]
            w = round_half_up(FIELD_WIDTH * s)
            self._add(
                "field",
                row["label"],
                [right - w, mid - h // 2, w, h],
                FIRST_ROW_ID + index,
                field_text=config_model.display_text(value),
            )
        elif kind == "slider":
            h = e["controls"]["slider"]["height"]["device_rounded"]
            w = round_half_up(SLIDER_WIDTH * s)
            self._add(
                "slider",
                row["label"],
                [right - w, mid - h // 2, w, h],
                FIRST_ROW_ID + index,
                value=float(value),
                min=0.5,
                max=1.0,
            )
        else:
            self._segments(index, row, value, right, mid)

    def _segments(self, index, row, value, right, mid):
        s, e = self.s, self.e
        px = e["controls"]["segment"]["text_px"] * s
        seg_h = e["controls"]["segment"]["height"]["device_rounded"]
        pill_h = device(e, "tab_pill_height")
        pad, gap = round_half_up(TRAY_PAD * s), round_half_up(SEGMENT_GAP * s)
        widths = [option_width(name, px) for name in row["options"]]
        total = sum(widths) + (len(widths) - 1) * gap + 2 * pad
        left = right - total
        self.cards[-1]["tray"] = [left, mid - pill_h // 2, total, pill_h]
        chosen = self._chosen(row, value)
        at = left + pad
        for place, (name, w) in enumerate(zip(row["options"], widths)):
            self._add(
                "segment",
                name,
                [at, mid - seg_h // 2, w, seg_h],
                FIRST_OPTION_ID + 10 * index + place,
                group=f"row:{row['key']}",
                axis="horizontal",
                checked=name == chosen,
            )
            at += w + gap

    @staticmethod
    def _chosen(row, value):
        """The option a stored value stands for: the option whose set-value maps to it."""
        return config_model.option_for(row, value)

    # -- scrolling ---------------------------------------------------------------------------------------------

    def _apply_scroll(self):
        for control in self.controls:
            if not control["header"]:
                control["box"] = [
                    control["box"][0],
                    control["box"][1] - self.scroll,
                    control["box"][2],
                    control["box"][3],
                ]
                self._shift_content(control)
        for card in self.cards:
            card["box"] = [
                card["box"][0],
                card["box"][1] - self.scroll,
                card["box"][2],
                card["box"][3],
            ]
            if "tray" in card:
                card["tray"] = [
                    card["tray"][0],
                    card["tray"][1] - self.scroll,
                    card["tray"][2],
                    card["tray"][3],
                ]
        for label in self.labels:
            label["box"] = [
                label["box"][0],
                label["box"][1] - self.scroll,
                label["box"][2],
                label["box"][3],
            ]
        for control in self.controls:
            _, y, _, h = control["box"]
            control["visible"] = control["header"] or (
                y >= self.body[1] and y + h <= self.body[1] + self.body[3]
            )

    def _shift_content(self, control):
        for key in ("label_box", "icon"):
            if control[key]:
                box = control[key]
                control[key] = [box[0], box[1] - self.scroll, box[2], box[3]]

    # -- landmarks ---------------------------------------------------------------------------------------------

    def landmarks(self):
        marks = [
            ("settings-panel", self.panel),
            ("settings-header", self.header),
            ("settings-tab-pill", self.pill),
        ]
        marks += [("settings-group-label", label["box"]) for label in self.labels]
        s, e = self.s, self.e
        pad = device(e, "card_padding_y")
        for card in self.cards:
            marks.append(("settings-card", card["box"]))
        for card in self.cards:
            top = card["box"][1] + 1 + pad
            left = card["text_left"]
            title_h, desc_h = (
                round_half_up(TITLE_LINE * s),
                round_half_up(DESCRIPTION_LINE * s),
            )
            width = card["box"][0] + card["box"][2] - left - 1
            marks.append(("settings-card-title", [left, top, width, title_h]))
            marks.append(
                (
                    "settings-card-description",
                    [
                        left,
                        top + title_h + device(e, "title_gap"),
                        width,
                        desc_h * len(card["lines"]),
                    ],
                )
            )
        return marks


# ------------------------------------------------------------------------------------------------ drawing


class ClippedCanvas(chrome_draw.Canvas):
    """A canvas that drops the part of a drawing that lies outside it.

    A scrolled page has cards and text past the window's edge; the base canvas would fail on them, and a real window
    simply does not show them.
    """

    def composite(self, left, top, mask, colour, alpha):
        height, width = mask.shape
        first_row, first_column = max(-top, 0), max(-left, 0)
        last_row = min(height, self.height - top)
        last_column = min(width, self.width - left)
        if last_row <= first_row or last_column <= first_column:
            return
        super().composite(
            left + first_column,
            top + first_row,
            mask[first_row:last_row, first_column:last_column],
            colour,
            alpha,
        )


def draw_text(canvas, colour, left, top, text, role, expected, weight, muted=False):
    px = expected["cap_heights"][role]["size_px"] * expected["scale"]
    canvas.text(left, top + ascent(weight, px), text, weight, px, colour)


def draw_page(canvas, theme, expected, settings, states, keyboard):
    """The panel, the page's cards and controls (clipped to the body) and then the header, over a canvas that holds the window."""
    s, e = settings.s, expected
    panel, body, header = settings.panel, settings.body, settings.header
    draw_floating.shadow(canvas, e, panel)
    radius = round_half_up(12 * s)
    canvas.rounded(panel, radius, theme.rgb["background"])
    canvas.rounded(panel, radius, theme.border[0], theme.border[1], outline=1)
    bottom = body[1] + body[3] - 1
    below = canvas.array[bottom:, :].copy()
    for label in settings.labels:
        if label["box"][1] >= bottom:
            continue
        draw_text(
            canvas,
            theme.rgb["muted_foreground"],
            label["box"][0],
            label["box"][1],
            label["text"],
            "text_ui",
            e,
            500,
        )
    pill_radius = e["controls"]["button"]["radius"]["device"]
    card_fill = chrome_draw.blend(
        theme.rgb["background"],
        theme.rgb["card"],
        e["settings"]["card_opacity"]["value"],
    )
    card_radius = e["settings"]["card_radius"]["device"]
    for card in settings.cards:
        box = card["box"]
        if box[1] >= bottom:
            continue
        canvas.rounded(box, card_radius, card_fill)
        canvas.rounded(
            box,
            card_radius,
            theme.border[0],
            e["settings_card_border_share"],
            outline=1,
        )
        title_top = box[1] + 1 + device(e, "card_padding_y")
        draw_text(
            canvas,
            theme.rgb["foreground"],
            card["text_left"],
            title_top,
            card["row"]["label"],
            "text_title",
            e,
            500,
        )
        desc_top = title_top + round_half_up(TITLE_LINE * s) + device(e, "title_gap")
        for number, line in enumerate(card["lines"]):
            draw_text(
                canvas,
                theme.rgb["muted_foreground"],
                card["text_left"],
                desc_top + number * round_half_up(DESCRIPTION_LINE * s),
                line,
                "text_desc",
                e,
                400,
            )
        if "tray" in card:
            canvas.rounded(card["tray"], pill_radius, theme.rgb["card"])
    draw_controls(canvas, theme, e, settings, states, keyboard, header=False)
    canvas.array[bottom:, :] = below
    canvas.rect(
        [header[0] + 1, header[1] + 1, header[2] - 2, header[3] - 1],
        theme.rgb["background"],
    )
    canvas.rect(
        [header[0] + 1, header[1] + header[3] - 1, header[2] - 2, 1],
        theme.border[0],
        theme.border[1],
    )
    canvas.rounded(settings.pill, pill_radius, theme.rgb["card"])
    draw_controls(canvas, theme, e, settings, states, keyboard, header=True)
    return canvas


def draw_controls(canvas, theme, e, settings, states, keyboard, header):
    s = settings.s
    for control in settings.controls:
        if control["header"] != header or (
            not header and control["box"][1] >= settings.body[1] + settings.body[3] - 1
        ):
            continue
        x, y = control["box"][:2]
        state = states.get(control["id"], "rest")
        container = canvas.array[
            y + control["box"][3] // 2, max(x - 1 - round_half_up(2 * s) - 2, 0)
        ].copy()
        chrome_draw.draw_control(canvas, theme, e, control, state, keyboard, container)
        if control["kind"] == "field":
            px = e["controls"]["field"]["text_px"] * s
            baseline = (
                y
                + control["box"][3] / 2
                + chrome_draw.INTER_CAP / chrome_draw.INTER_UNITS * px / 2
            )
            canvas.text(
                x + round_half_up(8 * s),
                baseline,
                control["field_text"],
                400,
                px,
                theme.rgb["foreground"],
            )
