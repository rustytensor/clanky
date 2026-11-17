#!/usr/bin/env python3
"""Reads a screenshot of a freshly started terminal window: is the prompt row inked, and how often does g1-ok show?

Usage:
  python3 check_wayland.py prompt SCREENSHOT.png          the prompt row has ink before anything is typed
  python3 check_wayland.py text SCREENSHOT.png COUNT [--exactly]   g1-ok shows at least (or exactly) COUNT times
  python3 check_wayland.py draw OUT.png [--fault cursor-only]    a window as it should look, or the editor's: a bare cursor
  python3 check_wayland.py window                          reads `swaymsg -t get_tree` JSON on stdin, prints "X Y W H SHELL"

Text is found with the cell geometry of expected.py (scale 1) and check_cells.py's reader: a cell matches a character
when its ink and the font's own glyph for it, drawn at that cell, cover each other to within one pixel. The search
therefore finds the word wherever the shell's prompt puts it. Output is "ok: ..." or "FAIL: ...", exit 1 on failure.
"""

import argparse
import json
import sys
from pathlib import Path

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
    commands.add_parser("window")
    arguments = parser.parse_args()
    handlers = {
        "prompt": command_prompt,
        "text": command_text,
        "draw": command_draw,
        "window": command_window,
    }
    sys.exit(handlers[arguments.command](arguments))


if __name__ == "__main__":
    main()
