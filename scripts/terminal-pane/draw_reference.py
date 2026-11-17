#!/usr/bin/env python3
"""Draws a "perfect" screenshot of the terminal grid fixture from the expected cells, with Pillow.

Usage: python3 scripts/terminal-pane/draw_reference.py EXPECTED.json OUT.png [--fault NAME] [--cursor ROW,COL,SHAPE]
                                            [--selection ROW,COL_FROM,COL_TO,#rrggbb]

The picture lets check_cells.py be shown to pass on a correct image and to fail on broken ones. Faults: drift,
late (one glyph 1 px right of its cell), background, nounderline, nobold (BOLD in the regular face), boxgap,
halfblock. Glyphs are drawn at their cell's left edge on the cell's baseline from expected.py; block elements are
exact rectangles, as a renderer draws them.
"""

import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

from gpui_coverage import corrected_mask
from PIL import Image, ImageDraw, ImageFont

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
FONT_REGULAR = REPO_ROOT / "assets/fonts/JetBrainsMono-Regular.ttf"
FONT_BOLD = REPO_ROOT / "assets/fonts/JetBrainsMono-Bold.ttf"
FONT_ITALIC = REPO_ROOT / "assets/fonts/JetBrainsMono-Italic.ttf"
FONT_BOLD_ITALIC = REPO_ROOT / "assets/fonts/JetBrainsMono-BoldItalic.ttf"
THEME_FILE = SCRIPT_DIR.parents[1] / "assets/themes/graphite-dark.json"
FONT_SIZE_LOGICAL_PX = 16  # the fixture's font size
UNITS_PER_EM = 1000
DIM_OPACITY = 0.7  # dim text is the foreground at this opacity over the background
UNDERLINE_BELOW_BASELINE = 1.5  # in logical pixels: centre of the underline stroke
UNDERCURL_BELOW_BASELINE = 1.2  # centre line of the wavy underline
UNDERCURL_AMPLITUDE = 0.9  # logical pixels
STRIKE_ABOVE_BASELINE = 4.8  # logical pixels: about the middle of the x-height
CURL_SUPERSAMPLE = 4
GLYPH_SUPERSAMPLE = 8
SHADE_OPACITY = {"░": 0.25, "▒": 0.5, "▓": 0.75}
HALF_BLOCKS = {
    "▀": (0, 0, 1, 0.5),
    "▄": (0, 0.5, 1, 1),
    "▌": (0, 0, 0.5, 1),
    "▐": (0.5, 0, 1, 1),
    "█": (0, 0, 1, 1),
}
HALFBLOCK_FAULT_FILL = 0.6
FAULTS = ("drift", "late", "background", "nounderline", "nobold", "boxgap", "halfblock")
CURSOR_COLOUR = (230, 230, 230)
CURSOR_STRIP_LOGICAL_PX = 2


def find_cjk_font():
    """The system's Japanese monospace font, as the renderer under test would find it through fontconfig."""
    result = subprocess.run(
        ["fc-match", "-f", "%{file}", ":lang=ja:spacing=mono"],
        capture_output=True,
        text=True,
        check=False,
        timeout=20,
    )
    path = result.stdout.strip()
    if result.returncode != 0 or not path or not Path(path).exists():
        sys.exit(
            "draw_reference.py: no Japanese font found with fc-match; install a CJK monospace font"
        )
    return path


def text_font(bold, italic):
    return {
        (False, False): FONT_REGULAR,
        (True, False): FONT_BOLD,
        (False, True): FONT_ITALIC,
        (True, True): FONT_BOLD_ITALIC,
    }[bold, italic]


def hex_to_rgb(text):
    return tuple(int(text[index : index + 2], 16) for index in (1, 3, 5))


def blend(old, new, amount):
    return tuple(round(o + (n - o) * amount) for o, n in zip(old, new))


def fill_rect(pixels, size, box, colour, opacity=1.0):
    """Fill a rectangle with fractional edges: edge pixels are blended by how much of them the rectangle covers."""
    left, top, right, bottom = box
    for y in range(max(math.floor(top), 0), min(math.ceil(bottom), size[1])):
        cover_y = min(bottom, y + 1) - max(top, y)
        for x in range(max(math.floor(left), 0), min(math.ceil(right), size[0])):
            cover_x = min(right, x + 1) - max(left, x)
            if cover_x > 0 and cover_y > 0:
                pixels[x, y] = blend(pixels[x, y], colour, cover_x * cover_y * opacity)


def draw_glyph(image, char, font_path, origin, baseline, colour, size_px):
    """Paint one glyph with its pen at x = origin on the given baseline.

    It is drawn at eight times the size and reduced, so the glyph sits at the exact fractional position and font
    hinting does not move it: the picture shows where the glyphs belong, not what a rasteriser makes of them. Its
    coverage goes through gpui-ce's text curve (gpui_coverage.py), so the glyph carries the ink gpui-ce gives it.
    """
    factor = GLYPH_SUPERSAMPLE
    margin = 4
    x0, y0 = math.floor(origin) - margin, math.floor(baseline - size_px * 1.2) - margin
    width, height = (
        math.ceil(size_px * 2.2) + 2 * margin,
        math.ceil(size_px * 1.8) + 2 * margin,
    )
    mask = Image.new("L", (width * factor, height * factor), 0)
    font = ImageFont.truetype(str(font_path), size_px * factor)
    ImageDraw.Draw(mask).text(
        ((origin - x0) * factor, (baseline - y0) * factor),
        char,
        font=font,
        fill=255,
        anchor="ls",
    )
    image.paste(
        colour,
        (x0, y0),
        corrected_mask(mask.resize((width, height), Image.BOX), colour),
    )


def draw_curl(image, box, centre_y, amplitude, thickness, colour, cell_width):
    """A wavy line through the word's box, drawn at 4x and reduced for smooth edges."""
    left, top, right, bottom = box
    scale = CURL_SUPERSAMPLE
    x0, y0 = math.floor(left) - 1, math.floor(top)
    width, height = math.ceil(right) - x0 + 2, math.ceil(bottom) - y0 + 1
    mask = Image.new("L", (width * scale, height * scale), 0)
    draw = ImageDraw.Draw(mask)
    points = []
    steps = int((right - left) * scale)
    for step in range(steps + 1):
        x = left + step / scale
        y = centre_y + amplitude * math.sin(2 * math.pi * (x - left) / (2 * cell_width))
        points.append(((x - x0) * scale, (y - y0) * scale))
    draw.line(points, fill=255, width=max(round(thickness * scale), 1), joint="curve")
    mask = mask.resize((width, height), Image.BOX)
    image.paste(colour, (x0, y0), mask)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("expected", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--fault", choices=FAULTS)
    parser.add_argument(
        "--cursor", help="ROW,COL,SHAPE with SHAPE block, bar or underline"
    )
    parser.add_argument("--selection", help="ROW,COL_FROM,COL_TO,#rrggbb")
    arguments = parser.parse_args()

    expected = json.loads(arguments.expected.read_text(encoding="utf-8"))
    theme = json.loads(THEME_FILE.read_text(encoding="utf-8"))
    background = hex_to_rgb(theme["colors"]["background"])
    scale = expected["scale"]
    cell_width, line_height = expected["cell_width"], expected["line_height"]
    origin_x, origin_y = expected["origin"]
    size_px = FONT_SIZE_LOGICAL_PX * scale
    width = math.ceil(2 * origin_x + 200 * cell_width)
    height = math.ceil(2 * origin_y + 11 * line_height)
    image = Image.new("RGB", (width, height), background)
    pixels = image.load()
    cjk_path = find_cjk_font()
    cells = expected["cells"]

    selection = None
    if arguments.selection:
        row, first, last, colour_text = arguments.selection.split(",")
        selection = (int(row), int(first), int(last), hex_to_rgb(colour_text))

    # Backgrounds, then the selection over them.
    for cell in cells:
        if cell["bg"]:
            colour = hex_to_rgb(cell["bg"])
            if (
                arguments.fault == "background"
                and cell["row"] == 1
                and cell["col"] == 5
            ):
                colour = (colour[0] + (1 if colour[0] < 255 else -1),) + colour[1:]
            fill_rect(
                pixels,
                image.size,
                (
                    cell["left"],
                    cell["top"],
                    cell["left"] + cell["width"],
                    cell["top"] + cell["height"],
                ),
                colour,
            )
    if selection:
        row, first, last, colour = selection
        top = origin_y + row * line_height
        fill_rect(
            pixels,
            image.size,
            (
                origin_x + first * cell_width,
                top,
                origin_x + (last + 1) * cell_width,
                top + line_height,
            ),
            colour,
        )

    # Glyphs and text decorations.
    for cell in cells:
        char = cell["char"]
        if char == " ":
            continue
        left, top = cell["left"], cell["top"]
        right, bottom = left + cell["width"], top + cell["height"]
        baseline = cell["baseline"]
        colour = hex_to_rgb(cell["fg"])
        attributes = cell["attrs"]
        if "dim" in attributes:
            colour = blend(background, colour, DIM_OPACITY)
        if char in HALF_BLOCKS:
            fractions = list(HALF_BLOCKS[char])
            if arguments.fault == "halfblock" and char == "▀":
                fractions[3] = HALFBLOCK_FAULT_FILL
            box = (
                left + fractions[0] * cell["width"],
                top + fractions[1] * cell["height"],
                left + fractions[2] * cell["width"],
                top + fractions[3] * cell["height"],
            )
            fill_rect(pixels, image.size, box, colour)
        elif char in SHADE_OPACITY:
            fill_rect(
                pixels,
                image.size,
                (left, top, right, bottom),
                colour,
                SHADE_OPACITY[char],
            )
        else:
            font_path = (
                cjk_path
                if ord(char) >= 0x3000
                else text_font(
                    "bold" in attributes and arguments.fault != "nobold",
                    "italic" in attributes,
                )
            )
            pen = left + (cell["width"] - size_px) / 2 if ord(char) >= 0x3000 else left
            if arguments.fault == "drift" and cell["row"] == 0:
                pen = origin_x + cell["col"] * round(cell_width)
            if arguments.fault == "late" and cell["row"] == 0 and cell["col"] == 42:
                pen += 1
            draw_glyph(image, char, font_path, pen, baseline, colour, size_px)
        if "underline" in attributes and arguments.fault != "nounderline":
            centre = baseline + UNDERLINE_BELOW_BASELINE * scale
            fill_rect(
                pixels,
                image.size,
                (left, centre - scale / 2, right, centre + scale / 2),
                colour,
            )
        if "strikethrough" in attributes:
            centre = baseline - STRIKE_ABOVE_BASELINE * scale
            fill_rect(
                pixels,
                image.size,
                (left, centre - scale / 2, right, centre + scale / 2),
                colour,
            )
    curl = [c for c in cells if "undercurl" in c["attrs"]]
    if curl:
        left, right = curl[0]["left"], curl[-1]["left"] + curl[-1]["width"]
        top = curl[0]["top"]
        draw_curl(
            image,
            (left, top, right, top + line_height),
            curl[0]["baseline"] + UNDERCURL_BELOW_BASELINE * scale,
            UNDERCURL_AMPLITUDE * scale,
            scale,
            hex_to_rgb(curl[0]["fg"]),
            cell_width,
        )

    # The box characters' last pixel column is blanked by the boxgap fault.
    if arguments.fault == "boxgap":
        for cell in cells:
            if cell["row"] in (7, 8, 9) and cell["char"] != " ":
                last_column = math.ceil(cell["left"] + cell["width"] - 0.5) - 1
                for y in range(
                    math.ceil(cell["top"] - 0.5),
                    math.ceil(cell["top"] + cell["height"] - 0.5),
                ):
                    pixels[last_column, y] = background

    if arguments.cursor:
        row, column, shape = arguments.cursor.split(",")
        left, top = (
            origin_x + int(column) * cell_width,
            origin_y + int(row) * line_height,
        )
        strip = CURSOR_STRIP_LOGICAL_PX * scale
        boxes = {
            "block": (left, top, left + cell_width, top + line_height),
            "bar": (left, top, left + strip, top + line_height),
            "underline": (
                left,
                top + line_height - strip,
                left + cell_width,
                top + line_height,
            ),
        }
        fill_rect(pixels, image.size, boxes[shape], CURSOR_COLOUR)
    image.save(arguments.output)


if __name__ == "__main__":
    main()
