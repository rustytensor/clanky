#!/usr/bin/env python3
"""Expected cells for the terminal grid fixture (scripts/terminal-pane/fixture.sh), computed from the font and theme files.

Usage: python3 scripts/terminal-pane/expected.py --scale S > expected-S.json

This script must not depend on the program under test: it reads only the font file and the theme file.
"""

import argparse
import json
import math
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

REPO_ROOT = Path(__file__).resolve().parents[2]

# Font and size: the terminal font and the default size in the design rulebook's token table.
FONT_FILE = REPO_ROOT / "assets/fonts/JetBrainsMono-Regular.ttf"
FONT_SIZE_LOGICAL_PX = 16
# Line height factor: the design rulebook's token table.
LINE_HEIGHT_FACTOR = 1.2
# Pane padding on every side, in logical pixels: the design rulebook's token table.
PANE_PADDING_LOGICAL_PX = 12
# Colours: the Graphite dark theme, copied from the design-tokens branch.
THEME_FILE = Path(__file__).resolve().parent / "graphite-dark.json"

TRUECOLOUR_ROW = ["#ff0000", "#00ff00", "#0000ff", "#123456", "#abcdef", "#808080"]
ATTRIBUTE_WORDS = [
    ("BOLD", "bold"),
    ("DIM", "dim"),
    ("ITALIC", "italic"),
    ("UNDER", "underline"),
    ("CURL", "undercurl"),
    ("STRIKE", "strikethrough"),
]
WIDE_TEXT = "日本語テスト"
BOX_ROWS = ["┌──┬──┐", "│  │  │", "└──┴──┘"]
BLOCK_ROW = "█▀▄▌▐░▒▓"


def load_cell_width(scale):
    """Advance of 'm' in device pixels, from the hmtx and head tables, not rounded."""
    font = TTFont(FONT_FILE)
    glyph_name = font.getBestCmap()[ord("m")]
    advance = font["hmtx"][glyph_name][0]
    return advance / font["head"].unitsPerEm * FONT_SIZE_LOGICAL_PX * scale


def round_half_up(value):
    return math.floor(value + 0.5)


def fixture_rows(theme):
    """Each row is a list of (text, width in cells, fg, bg, attrs); a text of one cell per character unless wide."""
    foreground = theme["colors"]["foreground"]
    background = theme["colors"]["background"]
    ansi = theme["ansi"]
    rows = [[(ch, 1, foreground, None, []) for ch in "miW." * 50]]
    rows.append(
        [(" ", 1, foreground, ansi[n], []) for n in range(16) for _ in range(2)]
    )
    rows.append([("█", 1, ansi[n], None, []) for n in range(16) for _ in range(2)])
    rows.append(
        [
            (" ", 1, foreground, colour, [])
            for colour in TRUECOLOUR_ROW
            for _ in range(2)
        ]
    )
    rows.append([(ch, 1, background, foreground, []) for ch in "INVERSE"])
    words = []
    for index, (word, attribute) in enumerate(ATTRIBUTE_WORDS):
        if index:
            words.append((" ", 1, foreground, None, []))
        words.extend((ch, 1, foreground, None, [attribute]) for ch in word)
    rows.append(words)
    rows.append([(ch, 2, foreground, None, []) for ch in WIDE_TEXT])
    rows.extend([(ch, 1, foreground, None, []) for ch in text] for text in BOX_ROWS)
    rows.append([(ch, 1, foreground, None, []) for ch in BLOCK_ROW])
    return rows


def build(scale):
    theme = json.loads(THEME_FILE.read_text(encoding="utf-8"))
    cell_width = load_cell_width(scale)
    line_height = round_half_up(FONT_SIZE_LOGICAL_PX * LINE_HEIGHT_FACTOR * scale)
    x0 = y0 = math.floor(PANE_PADDING_LOGICAL_PX * scale)
    cells = []
    for row, items in enumerate(fixture_rows(theme)):
        column = 0
        for char, width, fg, bg, attrs in items:
            keep = char != " " or bg is not None
            if keep:
                cells.append(
                    {
                        "row": row,
                        "col": column,
                        "char": char,
                        "left": x0 + column * cell_width,
                        "top": y0 + row * line_height,
                        "width": width * cell_width,
                        "height": line_height,
                        "fg": fg,
                        "bg": bg,
                        "attrs": attrs,
                    }
                )
            column += width
    return {
        "scale": scale,
        "cell_width": cell_width,
        "line_height": line_height,
        "origin": [x0, y0],
        "cells": cells,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--scale",
        type=float,
        required=True,
        help="device pixels per logical pixel, such as 1 or 1.5",
    )
    arguments = parser.parse_args()
    if arguments.scale <= 0:
        sys.exit("expected.py: --scale must be greater than zero")
    json.dump(build(arguments.scale), sys.stdout)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
