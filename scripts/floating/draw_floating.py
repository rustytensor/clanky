"""Draws G3's floating screens (palette, agent picker, completion list, find bar) from expected_floating.py's values.

Used by simulated_floating.py to give the gates a window to run on before the real one exists, and by gate-floating.sh to
write the committed fixtures. The layout is a stand-in with the right sizes: every height, radius and shadow comes
from the expected values, the positions are this file's choice. Nothing here reads the program under test.
"""

import re
import sys
from pathlib import Path

import numpy as np

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(SCRIPT_DIR.parent / "window-chrome"))
import draw_chrome_reference as chrome_draw  # noqa: E402
import expected_floating  # noqa: E402

round_half_up = chrome_draw.round_half_up
box_of = chrome_draw.box_of

POPOVER_PADDING = 4  # logical px between a popover's edge and its rows
FIRST_ID = 100
PALETTE_COMMANDS = (
    "Split right",
    "Split down",
    "New tab",
    "Close pane",
    "Show or hide the tools panel",
    "Open settings",
    "Go to HIH",
)
AGENTS = ("Fake Tool", "Codex", "Claude")
COMPLETIONS = ("scripts/gate-menu.sh", "scripts/gate-mouse.sh")
PALETTE_FOOTER = "Enter runs · Esc closes"
PALETTE_PLACEHOLDER = "Type a command"
AGENT_PLACEHOLDER = "Start an agent"
FIND_PLACEHOLDER = "Find in this pane"
FIND_BUTTONS = ("Previous", "Next", "Close")
PALETTE_WIDTH = 560
AGENT_WIDTH = 400
COMPLETION_WIDTH = 320
FIND_FIELD_WIDTH = 200
FIND_GAP = 4
SHORT_ROW_PX = 2  # the planted fault: a palette row this much too short (device px)
BIG_TEXT_PX = 2  # the planted fault: palette row text this much too large (logical px)
INTER_CAP = 1490 / 2048


def filtered(master, query):
    """The master list's entries whose text contains the query, case folded, with their index in the master list."""
    needle = query.lower()
    return [
        (index, text) for index, text in enumerate(master) if needle in text.lower()
    ]


def palette_matches(master, query):
    """The palette's rows for a query (clanky_state::command_palette's rule): labels starting with it, then with a
    word starting with it, then holding it, ties in list order; then the offer to run it in the pane, whose index is
    one past the master list."""
    needle = query.strip().lower()

    def rank(text):
        text = text.lower()
        if text.startswith(needle):
            return 0
        if any(word.startswith(needle) for word in re.split(r"[^0-9a-z]+", text)):
            return 1
        return 2 if needle in text else None

    found = sorted(
        (rank(text), index, text)
        for index, text in enumerate(master)
        if rank(text) is not None
    )
    run = f"Run \u201c{query.strip()}\u201d in this pane"
    return [(index, text) for _, index, text in found] + [(len(master), run)]


def row_control(index, text, kind, box):
    return {
        "id": FIRST_ID + index,
        "kind": kind,
        "label": text,
        "box": box,
        "enabled": True,
        "popover": True,
        "group": "list",
        "axis": "vertical",
        "checked": None,
        "label_box": None,
        "icon": None,
    }


def list_popover(
    expected,
    screen,
    master,
    query,
    left,
    top,
    width,
    with_footer,
    placeholder,
    short_row=False,
):
    """A palette-like popover: an input row, rows, an optional footer. Returns the popover dict."""
    s = expected["scale"]
    floating = expected["floating"]
    pad = round_half_up(POPOVER_PADDING * s)
    kind = "palette-row"
    input_h = floating["palette_input"]["device_rounded"]
    row_h = floating["palette_row" if screen == "palette" else "agent_picker_row"][
        "device_rounded"
    ]
    footer_h = floating["palette_footer"]["device_rounded"] if with_footer else 0
    shown = filtered(master, query)
    if screen == "palette" and query.strip():
        shown = palette_matches(master, query)
    x = round_half_up(left * s)
    y = round_half_up(top * s)
    w = round_half_up(width * s)
    height = input_h + pad + len(shown) * row_h + pad + footer_h
    box = [x, y, w, height]
    items = []
    for place, (index, text) in enumerate(shown):
        shorten = SHORT_ROW_PX if short_row and place == 0 else 0
        row_box = [
            x + pad,
            y + input_h + pad + place * row_h,
            w - 2 * pad,
            row_h - shorten,
        ]
        items.append(row_control(index, text, kind, row_box))
    return {
        "screen": screen,
        "box": box,
        "items": items,
        "input": [x, y, w, input_h],
        "footer": [x, y + height - footer_h, w, footer_h] if with_footer else None,
        "placeholder": placeholder,
        "query": query,
        "footer_text": PALETTE_FOOTER if with_footer else None,
    }


def completion_popover(expected, opener, window, query_rows, overlap_opener=False):
    """The completion list next to its opener (below it, or above it when there is no room); fault: over the opener."""
    s = expected["scale"]
    pad = round_half_up(POPOVER_PADDING * s)
    row_h = expected["floating"]["completion_row"]["device_rounded"]
    width = round_half_up(COMPLETION_WIDTH * s)
    height = 2 * pad + len(query_rows) * row_h
    gap = round_half_up(4 * s)
    x, y = opener[0], opener[1] + opener[3] + gap
    if y + height > window[1]:
        y = opener[1] - gap - height
    if overlap_opener:
        y = opener[1] + opener[3] // 2
    box = [x, y, width, height]
    items = [
        row_control(
            index,
            text,
            "menu-item",
            [x + pad, y + pad + place * row_h, width - 2 * pad, row_h],
        )
        for place, (index, text) in enumerate(query_rows)
    ]
    return {
        "screen": "completion-list",
        "box": box,
        "items": items,
        "input": None,
        "footer": None,
    }


def find_popover(expected, pane, query):
    """The find bar over the pane's top right corner: a field and three icon buttons on a card."""
    s = expected["scale"]
    floating = expected["floating"]
    pad = round_half_up(POPOVER_PADDING * s)
    gap = round_half_up(FIND_GAP * s)
    field_h = floating["find_field"]["device_rounded"]
    button = floating["find_icon_button"]["device_rounded"]
    width = (
        pad
        + round_half_up(FIND_FIELD_WIDTH * s)
        + len(FIND_BUTTONS) * (gap + button)
        + pad
    )
    height = 2 * pad + field_h
    x = pane[0] + pane[2] - width - round_half_up(8 * s)
    y = pane[1] + round_half_up(8 * s)
    field = {
        "id": FIRST_ID,
        "kind": "field",
        "label": query,
        "box": [x + pad, y + pad, round_half_up(FIND_FIELD_WIDTH * s), field_h],
        "enabled": True,
        "popover": True,
        "group": None,
        "axis": None,
        "checked": None,
        "label_box": None,
        "icon": None,
    }
    items = [field]
    left = field["box"][0] + field["box"][2]
    for place, name in enumerate(FIND_BUTTONS):
        left += gap
        box = [left, y + pad + (field_h - button) // 2, button, button]
        items.append(
            {
                "id": FIRST_ID + 1 + place,
                "kind": "icon-button",
                "label": name,
                "box": box,
                "enabled": True,
                "popover": True,
                "group": "find",
                "axis": "horizontal",
                "checked": None,
                "label_box": None,
                "icon": [
                    box[0] + (button - round_half_up(14 * s)) // 2,
                    box[1] + (button - round_half_up(14 * s)) // 2,
                    round_half_up(14 * s),
                    round_half_up(14 * s),
                ],
            }
        )
        left += button
    return {
        "screen": "find-bar",
        "box": [x, y, width, height],
        "items": items,
        "input": None,
        "footer": None,
        "query": query,
    }


def shadow(canvas, expected, box):
    """The popover's shadow, composited black around and under the box, from the same model the gate expects."""
    s = expected["scale"]
    spec = expected["popover_shadow"]
    sigma = spec["sigma_logical"] * s
    reach = int(spec["offset_y"] * s + 3 * sigma)
    x0, y0 = max(box[0] - reach, 0), max(box[1] - reach, 0)
    x1, y1 = (
        min(box[0] + box[2] + reach, canvas.width),
        min(box[1] + box[3] + reach, canvas.height),
    )
    shadow_y0 = box[1] + spec["offset_y"] * s
    xs = np.array(
        [
            expected_floating.shadow_share(x + 0.5, box[0], box[0] + box[2], sigma)
            for x in range(x0, x1)
        ]
    )
    ys = np.array(
        [
            expected_floating.shadow_share(
                y + 0.5, shadow_y0, shadow_y0 + box[3], sigma
            )
            for y in range(y0, y1)
        ]
    )
    alpha = spec["black_alpha"] * np.outer(ys, xs)
    region = canvas.array[y0:y1, x0:x1]
    region[:] = region * (1 - alpha[..., None])


def draw_card(canvas, theme, expected, box):
    radius = expected["floating"]["popover_radius"]["device"]
    canvas.rounded(box, radius, theme.rgb["card"])
    canvas.rounded(
        box,
        radius,
        theme.border[0],
        theme.border[1],
        outline=expected["floating"]["popover_border"]["device"],
    )


def draw_text(canvas, theme, expected, left, box, text, role, muted=False, extra_px=0):
    px = (expected["cap_heights"][role]["size_px"] + extra_px) * expected["scale"]
    baseline = box[1] + box[3] / 2 + INTER_CAP * px / 2
    colour = theme.rgb["muted_foreground" if muted else "foreground"]
    canvas.text(left, baseline, text, 400, px, colour)


def draw_rows(canvas, theme, expected, popover, states, keyboard, fault=None):
    s = expected["scale"]
    for control in popover["items"]:
        spec = expected["controls"][control["kind"]]
        state = states.get(control["id"], "rest")
        if control["kind"] in ("field", "icon-button"):
            # Not rows: they take the chrome's drawing, with its focus ring and border.
            chrome_draw.draw_control(
                canvas, theme, expected, control, state, keyboard, theme.rgb["card"]
            )
            continue
        box = control["box"]
        radius = spec["radius"]["device"]
        if state in ("hover", "pressed", "focus"):
            canvas.rounded(
                box,
                radius,
                theme.rgb["pressed"] if state == "pressed" else theme.rgb["accent"],
            )
        if state == "focus" and keyboard:
            canvas.ring_outside(
                box,
                radius,
                round_half_up(2 * s),
                theme.rgb["ring"],
                expected["states"]["focus"]["ring_alpha"],
            )
            canvas.rounded(box, radius, theme.rgb["ring"], 1.0, outline=1)
        role = spec["text_role"]
        draw_text(
            canvas,
            theme,
            expected,
            box[0] + round_half_up(8 * s),
            box,
            control["label"],
            role,
            extra_px=BIG_TEXT_PX
            if fault == "palette-text-big" and popover["screen"] == "palette"
            else 0,
        )


def draw_popover(canvas, theme, expected, popover, states, keyboard, fault=None):
    """The whole popover on a canvas that already holds the window: shadow, card, input, rows, footer."""
    box = popover["box"]
    shadow(canvas, expected, box)
    draw_card(canvas, theme, expected, box)
    s = expected["scale"]
    if popover.get("input"):
        inp = popover["input"]
        text = popover["query"] or popover["placeholder"]
        draw_text(
            canvas,
            theme,
            expected,
            inp[0] + round_half_up(12 * s),
            inp,
            text,
            "text_title",
            muted=not popover["query"],
        )
        canvas.rect(
            [inp[0] + 1, inp[1] + inp[3] - 1, inp[2] - 2, 1],
            theme.border[0],
            theme.border[1],
        )
    if popover.get("footer"):
        foot = popover["footer"]
        canvas.rect(
            [foot[0] + 1, foot[1], foot[2] - 2, 1], theme.border[0], theme.border[1]
        )
        draw_text(
            canvas,
            theme,
            expected,
            foot[0] + round_half_up(12 * s),
            foot,
            popover["footer_text"],
            "text_status",
            muted=True,
        )
    if popover["screen"] == "find-bar":
        field = popover["items"][0]
        text = popover["query"] or FIND_PLACEHOLDER
        draw_text(
            canvas,
            theme,
            expected,
            field["box"][0] + round_half_up(8 * s),
            field["box"],
            text,
            "text_ui",
            muted=not popover["query"],
        )
    draw_rows(canvas, theme, expected, popover, states, keyboard, fault)


def landmarks_of(popover):
    """The landmark lines a popover adds to the record: its input and footer boxes."""
    marks = []
    if popover.get("input"):
        marks.append(("popover-input", popover["input"]))
    if popover.get("footer"):
        marks.append(("popover-footer", popover["footer"]))
    return marks
