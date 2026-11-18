#!/usr/bin/env python3
"""Reads a screenshot of a freshly started terminal window: is the prompt row inked, and how often does g1-ok show?

Usage:
  python3 check_wayland.py prompt SCREENSHOT.png          the prompt row has ink before anything is typed
  python3 check_wayland.py text SCREENSHOT.png COUNT [--exactly]   g1-ok shows at least (or exactly) COUNT times
  python3 check_wayland.py draw OUT.png [--fault cursor-only]    a window as it should look, or the editor's: a bare cursor
  python3 check_wayland.py input SCREENSHOT.png           the default window's command input line has ink before typing
  python3 check_wayland.py typed SCREENSHOT.png           `echo g1-ok` shows in the input line, and nowhere above it
  python3 check_wayland.py output SCREENSHOT.png          after Enter: g1-ok twice in the focused pane (the command in
                                                          its block's command row, the output), nowhere else
  python3 check_wayland.py draw-default OUT.png [--stage input|typed|output] [--fault typed-not-sent]
                                                          the default window as it should look at each stage
  python3 check_wayland.py window                          reads `swaymsg -t get_tree` JSON on stdin, prints "X Y W H SHELL"
  python3 check_wayland.py app-id                          reads `swaymsg -t get_tree` JSON on stdin: the window's app_id is "clanky"
  python3 check_wayland.py wm-class                        reads `xprop WM_CLASS` output on stdin: the class is "clanky"
  python3 check_wayland.py draw-tree OUT.json [--fault app-id-unset]    a sway tree with one window, or one without an app_id
  python3 check_wayland.py draw-xprop OUT.txt [--fault app-id-unset]    xprop's answer for the window, or "not found"

The default window (command input, two panes, tools panel) is read by its own layout, from the rulebook and not from
the program: the focused pane is the lower one, its command bar's hairline lies 75 px above the pane's inside bottom
(the q2 gate's A2; the pane's inside bottom is the window's height minus the status line, the pane margin and the
pane's border), and the input line lies 38 to 72 px below that hairline. g1-ok is found in a band of rows by trying
every offset of the cell grid, as the text on those rows is not on the fixture's grid.

Text is found with the cell geometry of expected.py (scale 1) and check_cells.py's reader: a cell matches a character
when its ink and the font's own glyph for it, drawn at that cell, cover each other to within one pixel. The search
therefore finds the word wherever the shell's prompt puts it. Output is "ok: ..." or "FAIL: ...", exit 1 on failure.
"""

import argparse
import json
import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_cells  # noqa: E402
import expected  # noqa: E402

WORD = "g1-ok"
SCALE = 1
SEARCH_ROWS = 12  # the prompt and its output sit at the top of a fresh window
GLYPH_SUPERSAMPLE = 8
MATCH_MIN_COVERAGE = (
    0.75  # share of the font's glyph ink the screenshot must show, within one pixel
)
MATCH_MIN_PRECISION = 0.75  # share of the screenshot's ink that lies on the font's glyph, within one pixel
PROMPT_INK_MIN_PIXELS = (
    6  # the prompt row shows at least this many pixels that differ from the background
)
BLOCK_RIM_PX = 2  # a filled block is found by 5 x 5 neighbourhoods and its rim
FAULT_WINDOW_SIZE = (960, 400)


def theme_background():
    theme = json.loads(check_cells.THEME_FILE.read_text(encoding="utf-8"))
    return check_cells.hex_to_rgb(theme["colors"]["background"])


def glyph_mask(char, cell_width, line_height, ascender):
    """The font's glyph as the set of pixels it inks inside one cell, drawn large and reduced so it carries no hinting."""
    width = round(cell_width)
    large = ImageFont.truetype(
        str(check_cells.FONT_REGULAR),
        expected.FONT_SIZE_LOGICAL_PX * SCALE * GLYPH_SUPERSAMPLE,
    )
    canvas = Image.new(
        "L", (width * GLYPH_SUPERSAMPLE, line_height * GLYPH_SUPERSAMPLE), 0
    )
    ImageDraw.Draw(canvas).text(
        (0, ascender * GLYPH_SUPERSAMPLE), char, font=large, fill=255, anchor="ls"
    )
    small = canvas.resize((width, line_height), Image.BOX).load()
    return {
        (x, y)
        for x in range(width)
        for y in range(line_height)
        if small[x, y] > check_cells.INK_THRESHOLD
    }


def dilate(pixels):
    return {
        (x + dx, y + dy) for x, y in pixels for dx in (-1, 0, 1) for dy in (-1, 0, 1)
    }


def cell_ink(shot, left, top, width, height, background):
    return {
        (x, y)
        for x in range(width)
        for y in range(height)
        if shot.is_ink(left + x, top + y, background)
    }


def cell_matches(ink, mask):
    if not ink or not mask:
        return False
    coverage = len(mask & dilate(ink)) / len(mask)
    precision = len(ink & dilate(mask)) / len(ink)
    return coverage >= MATCH_MIN_COVERAGE and precision >= MATCH_MIN_PRECISION


def count_word(shot, geometry, background):
    """How many times WORD shows in the first rows, matching each cell against the font's glyph."""
    cell_width, line_height = geometry["cell_width"], geometry["line_height"]
    origin_x, origin_y = geometry["origin"]
    ascender = check_cells.Font(SCALE).ascender
    masks = {
        char: glyph_mask(char, cell_width, line_height, ascender) for char in set(WORD)
    }
    columns = int((shot.width - origin_x) / cell_width)
    count = 0
    for row in range(SEARCH_ROWS):
        top = round(origin_y + row * line_height)
        inked = [
            cell_ink(
                shot,
                round(origin_x + column * cell_width),
                top,
                round(cell_width),
                line_height,
                background,
            )
            for column in range(columns)
        ]
        column = 0
        while column + len(WORD) <= columns:
            if all(
                cell_matches(inked[column + i], masks[char])
                for i, char in enumerate(WORD)
            ):
                count += 1
                column += len(WORD)
            else:
                column += 1
    return count


def prompt_ink_pixels(shot, geometry, background):
    """Ink in the first row outside filled blocks: a block cursor is not a prompt.

    A pixel whose whole 5 x 5 neighbourhood is ink lies inside a filled block; the block and its 2 pixel rim are left
    out. Letter strokes are too thin to contain such a pixel.
    """
    origin_x, origin_y = geometry["origin"]
    ink = {
        (x, y)
        for x in range(origin_x, shot.width - origin_x)
        for y in range(origin_y, origin_y + geometry["line_height"])
        if shot.is_ink(x, y, background)
    }
    offsets = range(-BLOCK_RIM_PX, BLOCK_RIM_PX + 1)
    inside = {
        (x, y)
        for x, y in ink
        if all((x + dx, y + dy) in ink for dx in offsets for dy in offsets)
    }
    rim = {(x + dx, y + dy) for x, y in inside for dx in offsets for dy in offsets}
    return len(ink - rim)


# The default window, from the rulebook's token table (window-chrome/rulebook-tokens.json) and the q2 gate's A2.
TOKENS_FILE = (
    Path(__file__).resolve().parents[1] / "window-chrome" / "rulebook-tokens.json"
)
HAIRLINE_ABOVE_INSIDE_BOTTOM_PX = 75
HAIRLINE_TOLERANCE_PX = 3
HAIRLINE_COVERAGE = 0.9
HAIRLINE_LEVEL_TOLERANCE = 6
INPUT_BAND_PX = (38, 72)  # below the hairline: the input line, under the chips row
OUTPUT_ABOVE_HAIRLINE_PX = (220, 4)  # the pane's block area, above its command bar
INPUT_MIN_INK_PIXELS = (
    30  # a chevron alone is about 14; a caret block or placeholder text is far more
)
INK_SEARCH_COLUMNS = (
    0.19,
    0.95,
)  # the share of the window's width the hairline and bands are read across: from 243 px at 1280, right of the tools
# panel and its divider (237) and left of the flush pane's text (249)


def tokens():
    return json.loads(TOKENS_FILE.read_text(encoding="utf-8"))


def inside_bottom(shot):
    """The row under the lower pane's inside: the window height minus the status line and the pane margin. Panes are
    flush (rulebook, "R1 flush"): the margin is 0 and a pane has no border of its own."""
    book = tokens()
    chrome = book["heights"]["h_status"]["px"] + book["pane"]["margin"]["px"]
    return shot.height - chrome * SCALE


def hairline_colour(background):
    alpha = tokens()["pane"]["border_alpha"]["value"]
    return tuple(c + alpha * (255 - c) for c in background)


def hairlines(shot, background):
    """The rows that are a command bar's hairline: the border colour across 90 % of the pane columns."""
    line = hairline_colour(background)
    columns = range(
        int(INK_SEARCH_COLUMNS[0] * shot.width),
        int(INK_SEARCH_COLUMNS[1] * shot.width),
        3,
    )
    found = []
    for y in range(shot.height):
        near = sum(
            shot.difference(x, y, line) <= HAIRLINE_LEVEL_TOLERANCE for x in columns
        )
        if near >= HAIRLINE_COVERAGE * len(columns):
            found.append(y)
    return found


def lower_hairline(shot, background):
    """The lower pane's hairline, or None; it must lie where the rulebook puts it."""
    want = inside_bottom(shot) - HAIRLINE_ABOVE_INSIDE_BOTTOM_PX
    near = [
        y for y in hairlines(shot, background) if abs(y - want) <= HAIRLINE_TOLERANCE_PX
    ]
    return (near[0], want) if near else (None, want)


def band_ink_pixels(shot, background, top, bottom):
    left, right = (int(share * shot.width) for share in INK_SEARCH_COLUMNS)
    return sum(
        shot.is_ink(x, y, background)
        for y in range(top, bottom)
        for x in range(left, right)
    )


def count_word_free(shot, geometry, background, top, bottom):
    """The most times WORD shows in the rows between top and bottom over every offset of the cell grid: the grid's
    origin is tried at every pixel of one cell before the first inked column, and its rows at every phase of a line,
    and only the rows and columns that hold ink are matched."""
    cell_width, line_height = geometry["cell_width"], geometry["line_height"]
    ascender = check_cells.Font(SCALE).ascender
    masks = {
        char: glyph_mask(char, cell_width, line_height, ascender) for char in set(WORD)
    }
    left = int(INK_SEARCH_COLUMNS[0] * shot.width)
    right = int(INK_SEARCH_COLUMNS[1] * shot.width)
    pixels = np.asarray(shot.image, dtype=int)[top:bottom, left:right]
    ink = np.abs(pixels - np.array(background)).max(axis=2) > check_cells.INK_THRESHOLD
    if not ink.any():
        return 0
    inked_columns = np.where(ink.any(axis=0))[0]
    first, last = int(inked_columns.min()), int(inked_columns.max())
    width = round(cell_width)
    best = 0
    for phase in range(line_height):
        rows = [
            r
            for r in range(phase, ink.shape[0] - line_height + 1, line_height)
            if ink[r : r + line_height].any()
        ]
        if not rows:
            continue
        for shift in range(width):
            origin = first - shift
            columns = int((last + 1 - origin) / cell_width) + 1
            count = 0
            for row in rows:
                cells = []
                for column in range(columns):
                    x0 = round(origin + column * cell_width)
                    cell = ink[row : row + line_height, max(x0, 0) : x0 + width]
                    cells.append({(x, y) for y, x in zip(*np.nonzero(cell))})
                column = 0
                while column + len(WORD) <= columns:
                    if all(
                        cell_matches(cells[column + i], masks[char])
                        for i, char in enumerate(WORD)
                    ):
                        count += 1
                        column += len(WORD)
                    else:
                        column += 1
            best = max(best, count)
    return best


def load_shot(path):
    background = theme_background()
    return check_cells.Shot(Image.open(path), background), background


def command_prompt(arguments):
    shot, background = load_shot(arguments.screenshot)
    pixels = prompt_ink_pixels(shot, expected.build(SCALE), background)
    if pixels < PROMPT_INK_MIN_PIXELS:
        print(
            f"FAIL: wayland prompt row shows {pixels} inked pixels outside the cursor before typing, at least {PROMPT_INK_MIN_PIXELS} expected"
        )
        return 1
    print(f"ok: wayland prompt row has ink ({pixels} pixels)")
    return 0


def command_text(arguments):
    shot, background = load_shot(arguments.screenshot)
    count = count_word(shot, expected.build(SCALE), background)
    if (count != arguments.count) if arguments.exactly else (count < arguments.count):
        relation = "exactly" if arguments.exactly else "at least"
        print(
            f"FAIL: wayland {WORD} shows {count} times, {relation} {arguments.count} expected"
        )
        return 1
    print(f"ok: wayland {WORD} shows {count} times")
    return 0


def default_layout(arguments):
    shot, background = load_shot(arguments.screenshot)
    hairline, want = lower_hairline(shot, background)
    return shot, background, hairline, want


def command_input(arguments):
    shot, background, hairline, want = default_layout(arguments)
    if hairline is None:
        print(
            f"FAIL: wayland default window has no command bar hairline at y {want} (+-{HAIRLINE_TOLERANCE_PX}), 75 px above the lower pane's inside bottom"
        )
        return 1
    top, bottom = hairline + INPUT_BAND_PX[0], hairline + INPUT_BAND_PX[1]
    pixels = band_ink_pixels(shot, background, top, bottom)
    if pixels < INPUT_MIN_INK_PIXELS:
        print(
            f"FAIL: wayland input line (y {top}..{bottom}) shows {pixels} inked pixels before typing, at least {INPUT_MIN_INK_PIXELS} expected (a caret or placeholder)"
        )
        return 1
    print(
        f"ok: wayland input line has ink before typing ({pixels} pixels, hairline at y {hairline})"
    )
    return 0


def band_counts(arguments):
    """WORD counts in the lower pane's input line, its block area, and the upper pane's block area."""
    shot, background, hairline, want = default_layout(arguments)
    if hairline is None:
        return None, f"wayland default window has no command bar hairline at y {want}"
    geometry = expected.build(SCALE)
    above = (
        hairline - OUTPUT_ABOVE_HAIRLINE_PX[0],
        hairline - OUTPUT_ABOVE_HAIRLINE_PX[1],
    )
    upper = [y for y in hairlines(shot, background) if y < hairline - 100]
    counts = {
        "input line": count_word_free(
            shot,
            geometry,
            background,
            hairline + INPUT_BAND_PX[0],
            hairline + INPUT_BAND_PX[1],
        ),
        "lower pane": count_word_free(shot, geometry, background, *above),
    }
    if upper:
        counts["upper pane"] = count_word_free(
            shot,
            geometry,
            background,
            upper[-1] - OUTPUT_ABOVE_HAIRLINE_PX[0],
            upper[-1] - OUTPUT_ABOVE_HAIRLINE_PX[1],
        )
    return counts, ""


def command_typed(arguments):
    counts, problem = band_counts(arguments)
    if counts is None:
        print(f"FAIL: {problem}")
        return 1
    if counts["input line"] != 1 or counts["lower pane"] or counts.get("upper pane"):
        print(
            f"FAIL: wayland typed {WORD} should show once in the input line and nowhere above it, counted {counts}"
        )
        return 1
    print(f"ok: wayland the typed line shows in the input line ({counts})")
    return 0


def command_output(arguments):
    counts, problem = band_counts(arguments)
    if counts is None:
        print(f"FAIL: {problem}")
        return 1
    if counts["lower pane"] != 2 or counts["input line"] or counts.get("upper pane"):
        print(
            f"FAIL: wayland after Enter {WORD} should show twice in the focused pane (its command row and the output) and nowhere else, counted {counts}"
        )
        return 1
    print(f"ok: wayland after Enter {WORD} shows twice in the focused pane ({counts})")
    return 0


def command_draw(arguments):
    """A window as a classic prompt draws it after `echo g1-ok`; the fault is the editor's window: a block cursor only."""
    geometry = expected.build(SCALE)
    theme = json.loads(check_cells.THEME_FILE.read_text(encoding="utf-8"))
    image = Image.new("RGB", FAULT_WINDOW_SIZE, theme["colors"]["background"])
    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype(
        str(check_cells.FONT_REGULAR), expected.FONT_SIZE_LOGICAL_PX * SCALE
    )
    ascender = check_cells.Font(SCALE).ascender
    rows = {} if arguments.fault == "cursor-only" else {0: f"$ echo {WORD}", 1: WORD}
    for row, text in rows.items():
        for column, char in enumerate(text):
            x = geometry["origin"][0] + column * geometry["cell_width"]
            y = geometry["origin"][1] + row * geometry["line_height"] + ascender
            draw.text(
                (x, y), char, font=font, fill=theme["colors"]["foreground"], anchor="ls"
            )
    if arguments.fault == "cursor-only":
        left, top = geometry["origin"]
        draw.rectangle(
            (left, top, left + geometry["cell_width"], top + geometry["line_height"]),
            fill=theme["colors"]["foreground"],
        )
    image.save(arguments.out)
    return 0


APP_ID = "clanky"  # the window's application id: the desktop file's StartupWMClass and the icon name
DEFAULT_WINDOW_SIZE = (1280, 720)
# The default window's flush panes beside the tools panel (236 px and its divider), at 1280 x 720: the title bar
# (40) and its divider above, the status line (32) below, one divider row between the two 323 px panes.
PANE_LEFT, PANE_RIGHT = (237, 1279)
PANE_TOPS = (41, 365)  # logical px: the upper and lower pane's top edge
FRAME_GREY = 29


def command_draw_default(arguments):
    """The default window at one stage: the command input empty, with `echo g1-ok` typed, or after Enter. The fault
    leaves the typed line in the input line and never runs it (typing that does not reach the shell)."""
    geometry = expected.build(SCALE)
    theme = json.loads(check_cells.THEME_FILE.read_text(encoding="utf-8"))
    background = check_cells.hex_to_rgb(theme["colors"]["background"])
    image = Image.new("RGB", DEFAULT_WINDOW_SIZE, (FRAME_GREY,) * 3)
    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype(
        str(check_cells.FONT_REGULAR), expected.FONT_SIZE_LOGICAL_PX * SCALE
    )
    ascender = check_cells.Font(SCALE).ascender
    bottom = inside_bottom(Image_shot(image, background))
    pane_bottoms = (PANE_TOPS[1] - 2, bottom - 1)
    line = tuple(round(c) for c in hairline_colour(background))
    for top, end in zip(PANE_TOPS, pane_bottoms):
        draw.rectangle((PANE_LEFT, top, PANE_RIGHT, end), fill=background)
        draw.line(
            (
                PANE_LEFT + 1,
                end - 1 - HAIRLINE_ABOVE_INSIDE_BOTTOM_PX,
                PANE_RIGHT - 1,
                end - 1 - HAIRLINE_ABOVE_INSIDE_BOTTOM_PX,
            ),
            fill=line,
        )
    hair = bottom - HAIRLINE_ABOVE_INSIDE_BOTTOM_PX
    foreground = theme["colors"]["foreground"]

    def write(text, left, top):
        for column, char in enumerate(text):
            draw.text(
                (left + column * geometry["cell_width"], top + ascender),
                char,
                font=font,
                fill=foreground,
                anchor="ls",
            )

    input_top = hair + 44
    typed = arguments.stage == "typed" or arguments.fault == "typed-not-sent"
    draw.polygon(
        [(254, input_top + 4), (260, input_top + 10), (254, input_top + 16)],
        fill=(70, 110, 220),
    )
    if typed:
        write(f"echo {WORD}", 271, input_top)
    else:
        draw.rectangle((271, input_top, 281, input_top + 18), fill=foreground)
    if arguments.stage == "output" and arguments.fault != "typed-not-sent":
        write(f"echo {WORD}", 249, hair - 160)
        write(WORD, 249, hair - 160 + geometry["line_height"])
    image.save(arguments.out)
    return 0


class Image_shot:
    """Just the size of an image, for the layout arithmetic before the screenshot reader exists."""

    def __init__(self, image, background):
        self.width, self.height = image.size


def command_app_id(_arguments):
    window = find_window(json.load(sys.stdin))
    if window is None:
        print("FAIL: wayland sway shows no window to read an app_id from")
        return 1
    app_id = window.get("app_id")
    if app_id != APP_ID:
        print(f"FAIL: wayland the window's app_id is {app_id!r}, {APP_ID!r} expected")
        return 1
    print(f"ok: wayland the window's app_id is {APP_ID!r}")
    return 0


def command_wm_class(_arguments):
    text = sys.stdin.read().strip()
    match = re.fullmatch(r'WM_CLASS\(STRING\) = "([^"]*)", "([^"]*)"', text)
    if not match:
        print(f"FAIL: x11 WM_CLASS is not set on the window (xprop said {text!r})")
        return 1
    if match.groups() != (APP_ID, APP_ID):
        print(
            f"FAIL: x11 WM_CLASS is {match.groups()!r}, ({APP_ID!r}, {APP_ID!r}) expected"
        )
        return 1
    print(f"ok: x11 WM_CLASS is {APP_ID!r}")
    return 0


def command_draw_tree(arguments):
    window = {
        "type": "con",
        "pid": 4242,
        "shell": "xdg_shell",
        "rect": {"x": 0, "y": 0, "width": 1280, "height": 720},
        "nodes": [],
    }
    if arguments.fault != "app-id-unset":
        window["app_id"] = APP_ID
    else:
        window["app_id"] = None
    arguments.out.write_text(
        json.dumps({"nodes": [window], "floating_nodes": []}), encoding="utf-8"
    )
    return 0


def command_draw_xprop(arguments):
    text = (
        "WM_CLASS:  not found."
        if arguments.fault == "app-id-unset"
        else f'WM_CLASS(STRING) = "{APP_ID}", "{APP_ID}"'
    )
    arguments.out.write_text(text + "\n", encoding="utf-8")
    return 0


def find_window(node):
    """The first window in sway's tree: a leaf with a pid and a shell."""
    if (
        node.get("pid")
        and node.get("type") in ("con", "floating_con")
        and not node.get("nodes")
    ):
        return node
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        found = find_window(child)
        if found:
            return found
    return None


def command_window(_arguments):
    window = find_window(json.load(sys.stdin))
    if window is None:
        return 1
    rect = window["rect"]
    print(
        rect["x"],
        rect["y"],
        rect["width"],
        rect["height"],
        window.get("shell", "unknown"),
    )
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    prompt = commands.add_parser("prompt")
    prompt.add_argument("screenshot", type=Path)
    text = commands.add_parser("text")
    text.add_argument("screenshot", type=Path)
    text.add_argument("count", type=int)
    text.add_argument("--exactly", action="store_true")
    draw = commands.add_parser("draw")
    draw.add_argument("out", type=Path)
    draw.add_argument("--fault", choices=["cursor-only"])
    for name in ("input", "typed", "output"):
        commands.add_parser(name).add_argument("screenshot", type=Path)
    default = commands.add_parser("draw-default")
    default.add_argument("out", type=Path)
    default.add_argument(
        "--stage", choices=["input", "typed", "output"], default="output"
    )
    default.add_argument("--fault", choices=["typed-not-sent"])
    commands.add_parser("window")
    commands.add_parser("app-id")
    commands.add_parser("wm-class")
    for name in ("draw-tree", "draw-xprop"):
        drawing = commands.add_parser(name)
        drawing.add_argument("out", type=Path)
        drawing.add_argument("--fault", choices=["app-id-unset"])
    arguments = parser.parse_args()
    handlers = {
        "prompt": command_prompt,
        "text": command_text,
        "draw": command_draw,
        "input": command_input,
        "typed": command_typed,
        "output": command_output,
        "draw-default": command_draw_default,
        "app-id": command_app_id,
        "wm-class": command_wm_class,
        "draw-tree": command_draw_tree,
        "draw-xprop": command_draw_xprop,
        "window": command_window,
    }
    sys.exit(handlers[arguments.command](arguments))


if __name__ == "__main__":
    main()
