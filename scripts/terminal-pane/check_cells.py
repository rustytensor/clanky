#!/usr/bin/env python3
"""Screenshot checks for the terminal grid fixture: reads a screenshot and the expected cells, says pass or fail.

Usage: python3 scripts/terminal-pane/check_cells.py SCREENSHOT.png EXPECTED.json [--cursor ROW,COL,SHAPE]
                                         [--selection ROW,COL_FROM,COL_TO,#rrggbb]

EXPECTED.json comes from expected.py. The screenshot's (0, 0) is the pane's top-left corner. Prints one line per check,
`ok: <check> <details>` or `FAIL: <check> expected <x> observed <y>`, and exits 1 if any check failed.
This script depends on nothing but the screenshot, the expected cells, the theme file and the font file.
"""

import argparse
import json
import math
import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
FONT_REGULAR = REPO_ROOT / "assets/fonts/JetBrainsMono-Regular.ttf"
THEME_FILE = SCRIPT_DIR / "graphite-dark.json"
FONT_SIZE_LOGICAL_PX = 16  # the fixture's font size, the same as in expected.py

# Thresholds. Each is fixed here so that a failing check cannot be argued away by editing a call.
INK_THRESHOLD = 24  # a pixel is ink when it differs from the cell background by more than this on any channel
PLACEMENT_TOLERANCE_PX = 0.5  # glyph left, right and top edges may differ from the font's own metrics by this much
PLACEMENT_WINDOW_PX = 1  # the cell box is widened by this much on each side when looking for a glyph's left edge
BACKGROUND_TOLERANCE = (
    0  # background colours must be exact: the renderer draws them as flat fills
)
CORE_TOLERANCE = 3  # glyph core colours may differ by this much: anti-aliasing and colour management round
BOX_SHRINK_PX = (
    2  # colour medians are taken this far inside the cell box, away from blended edges
)
CORE_PIXEL_COUNT = 12  # the glyph core is the median of this many pixels closest to the expected colour
LINE_RUN_FRACTION = 0.8  # an underline or strikethrough is a horizontal ink run covering this share of the word
UNDERLINE_BAND_FRACTION = (
    0.25  # underlines are looked for in the bottom quarter of the cell
)
STRIKE_BAND = (0.3, 0.7)  # strikethroughs are looked for in the middle 40 % of the cell
UNDERCURL_COLUMN_FRACTION = (
    0.9  # a wavy underline has ink in nearly every column of the word
)
UNDERCURL_MIN_WAVE_PX = 0.6  # and its ink centre moves up and down by at least this much along the word (the cell has little room below the baseline)
BOLD_MIN_RATIO = (
    1.15  # bold letters carry at least this many times the ink of regular letters
)
BOLD_ABSENT_MAX_RATIO = 1.08  # a word that is not bold stays below this
ITALIC_MIN_SHIFT_PX = 1.0  # an italic word leans right: its top ink centre moves this much more than the same upright word's
REFERENCE_SUPERSAMPLE = 8  # the upright reference is drawn at this multiple and reduced, so it carries no hinting
DIM_MAX_LUMINANCE_RATIO = 0.75  # dim text is at least 25 % darker than the foreground
MIN_INK_PIXELS = 3  # a wide character's cell half counts as inked from this many pixels
HALF_BLOCK_TOLERANCE_PX = 1  # half blocks fill their half to within one pixel
BLOCK_COVERAGE_MIN = 0.7  # a block cursor inks this share of its cell (the glyph inside may match the background)
CURSOR_STRIP_ROW_FRACTION = (
    0.9  # a bar or underline cursor is ink along this share of the cell's length
)
CURSOR_STRIP_MIN_PX, CURSOR_STRIP_MAX_PX = 1, 3  # and is this many pixels thick
CURSOR_BLOCK_TOLERANCE_PX = 1
SELECTION_TOLERANCE = 3  # selection colour medians may differ by this much

BOX_EDGES = {  # which cell edges the line strokes of each box character must reach: left, right, top, bottom
    "┌": "rb",
    "┐": "lb",
    "└": "rt",
    "┘": "lt",
    "─": "lr",
    "│": "tb",
    "┬": "lrb",
    "┴": "lrt",
}
ATTRIBUTE_WORDS = ("bold", "dim", "italic", "underline", "undercurl", "strikethrough")


class Shot:
    """A screenshot with the helpers every check needs."""

    def __init__(self, image, background):
        self.image = image.convert("RGB")
        self.width, self.height = self.image.size
        self.pixels = self.image.load()
        self.background = background

    def get(self, x, y):
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.pixels[x, y]
        return self.background

    def difference(self, x, y, reference):
        """Largest channel difference from REFERENCE: a colour, or a function giving the colour at (x, y)."""
        if callable(reference):
            reference = reference(x, y)
        return max(abs(a - b) for a, b in zip(self.get(x, y), reference))

    def is_ink(self, x, y, reference):
        return self.difference(x, y, reference) > INK_THRESHOLD


def span(low, high):
    """Indices of the pixels whose centres lie in [low, high)."""
    return range(math.ceil(low - 0.5), math.ceil(high - 0.5))


def hex_to_rgb(text):
    return tuple(int(text[index : index + 2], 16) for index in (1, 3, 5))


def dominant_colour(colours):
    """The most common colour: a flat background stays exact even when a glyph covers half of the box."""
    counts = {}
    for colour in colours:
        counts[colour] = counts.get(colour, 0) + 1
    return max(counts, key=counts.get)


def median_colour(colours):
    return tuple(
        sorted(colour[channel] for colour in colours)[len(colours) // 2]
        for channel in range(3)
    )


def colour_distance(first, second):
    return max(abs(a - b) for a, b in zip(first, second))


def luminance(colour):
    return 0.2126 * colour[0] + 0.7152 * colour[1] + 0.0722 * colour[2]


def core_colour(shot, xs, ys, expected):
    """Median of the pixels closest to the expected colour: the glyph core, free of anti-aliased edges."""
    pixels = sorted(
        (shot.get(x, y) for x in xs for y in ys),
        key=lambda p: colour_distance(p, expected),
    )
    return median_colour(pixels[:CORE_PIXEL_COUNT])


def cell_box(cell):
    return (
        cell["left"],
        cell["top"],
        cell["left"] + cell["width"],
        cell["top"] + cell["height"],
    )


def cell_background(cell, theme_background):
    return hex_to_rgb(cell["bg"]) if cell["bg"] else theme_background


class Font:
    """The regular font's metrics at the screenshot's size."""

    def __init__(self, scale):
        self.table = TTFont(FONT_REGULAR)
        self.units = self.table["head"].unitsPerEm
        self.size = FONT_SIZE_LOGICAL_PX * scale
        self.ascender = self.table["hhea"].ascent / self.units * self.size
        self.cmap = self.table.getBestCmap()
        self.pillow = ImageFont.truetype(str(FONT_REGULAR), self.size)

    def left_bearing(self, char):
        return self.table["hmtx"][self.cmap[ord(char)]][1] / self.units * self.size

    def right_extent(self, char):
        return self.table["glyf"][self.cmap[ord(char)]].xMax / self.units * self.size

    def top_extent(self, char):
        return self.table["glyf"][self.cmap[ord(char)]].yMax / self.units * self.size


def left_edge(shot, xs, ys, background, full_scale):
    """Sub-pixel left edge of the ink: first inked column plus the share of it left uncovered."""
    for x in xs:
        strongest = max(shot.difference(x, y, background) for y in ys)
        if strongest > INK_THRESHOLD:
            return x + 1 - min(strongest / full_scale, 1.0)
    return None


def right_edge(shot, xs, ys, background, full_scale):
    for x in reversed(xs):
        strongest = max(shot.difference(x, y, background) for y in ys)
        if strongest > INK_THRESHOLD:
            return x + min(strongest / full_scale, 1.0)
    return None


def top_edge(shot, xs, ys, background, full_scale):
    for y in ys:
        strongest = max(shot.difference(x, y, background) for x in xs)
        if strongest > INK_THRESHOLD:
            return y + 1 - min(strongest / full_scale, 1.0)
    return None


def parse_selection(argument):
    """ROW,COL_FROM,COL_TO,#rrggbb as (row, first, last, colour), or None."""
    if not argument:
        return None
    row, first, last, colour_text = argument.split(",")
    return int(row), int(first), int(last), hex_to_rgb(colour_text)


def selected(cell, selection):
    if not selection:
        return False
    row, first, last, _ = selection
    return cell["row"] == row and first <= cell["col"] <= last


def ground_at(cells, theme_background, selection):
    """The colour ink is measured against at each pixel: the selection colour on the selected cells' pixels (a pixel
    belongs to a cell when its centre lies inside it), the theme background everywhere else."""
    chosen = [cell for cell in cells if selected(cell, selection)]
    if not chosen:
        return theme_background
    left, top = math.ceil(chosen[0]["left"] - 0.5), math.ceil(chosen[0]["top"] - 0.5)
    last = chosen[-1]
    right = math.ceil(last["left"] + last["width"] - 0.5)
    bottom = math.ceil(last["top"] + last["height"] - 0.5)
    colour = selection[3]
    return (
        lambda x, y: colour
        if left <= x < right and top <= y < bottom
        else theme_background
    )


def check_placement(shot, cells, font, theme_background, selection=None):
    worst_error, worst_column = 0.0, None
    missing, column_199 = [], None
    top_worst = 0.0
    row_cells = [cell for cell in cells if cell["row"] == 0]
    previous_ink_end = -1000.0
    background = ground_at(row_cells, theme_background, selection)
    for index, cell in enumerate(row_cells):
        left, top, right, bottom = cell_box(cell)
        under = selection[3] if selected(cell, selection) else theme_background
        full_scale = max(colour_distance(hex_to_rgb(cell["fg"]), under), 1)
        expected = left + font.left_bearing(cell["char"])
        # The pixels holding the previous glyph's own right edge are not this glyph's ink.
        window = [
            x
            for x in span(left - PLACEMENT_WINDOW_PX, right + PLACEMENT_WINDOW_PX)
            if x > math.floor(previous_ink_end)
        ]
        previous_ink_end = left + font.right_extent(cell["char"])
        measured = left_edge(shot, window, span(top, bottom), background, full_scale)
        # The right edge is measured too, so that a different glyph standing where this one belongs is not taken for it.
        next_start = (
            row_cells[index + 1]["left"]
            + font.left_bearing(row_cells[index + 1]["char"])
            if index + 1 < len(row_cells)
            else 1e9
        )
        right_window = [
            x
            for x in span(left, right + PLACEMENT_WINDOW_PX)
            if x < math.floor(next_start)
        ]
        measured_right = right_edge(
            shot, right_window, span(top, bottom), background, full_scale
        )
        if measured is None or measured_right is None:
            missing.append(cell["col"])
            error = math.inf
        else:
            error = max(
                abs(measured - expected),
                abs(measured_right - (left + font.right_extent(cell["char"]))),
            )
        if cell["col"] == 199:
            column_199 = "no ink" if math.isinf(error) else f"{error:.2f} px"
        if error > worst_error:
            worst_error, worst_column = error, cell["col"]
        if cell["char"] == "W":
            xs = span(left + font.left_bearing("W"), left + font.right_extent("W"))
            observed_top = top_edge(
                shot, xs, span(top - 2, bottom), background, full_scale
            )
            expected_top = cell["baseline"] - font.top_extent("W")
            top_error = (
                math.inf if observed_top is None else abs(observed_top - expected_top)
            )
            top_worst = max(top_worst, top_error)
    worst_text = "no ink" if math.isinf(worst_error) else f"{worst_error:.2f} px"
    detail = f"worst edge error {worst_text} at column {worst_column}; column 199 {column_199}; W top error {top_worst:.2f} px"
    if worst_error > PLACEMENT_TOLERANCE_PX or top_worst > PLACEMENT_TOLERANCE_PX:
        return (
            False,
            f"expected within {PLACEMENT_TOLERANCE_PX} px observed {detail} ({len(missing)} cells without ink)",
        )
    return True, detail


def check_colours(shot, cells, theme_background):
    failures = []
    checked = 0
    for cell in cells:
        left, top, right, bottom = cell_box(cell)
        row, expected_fg = cell["row"], cell["fg"] and hex_to_rgb(cell["fg"])
        if row in (1, 3, 4):
            inner = (
                span(left + BOX_SHRINK_PX, right - BOX_SHRINK_PX),
                span(top + BOX_SHRINK_PX, bottom - BOX_SHRINK_PX),
            )
            observed = dominant_colour(
                [shot.get(x, y) for x in inner[0] for y in inner[1]]
            )
            expected = hex_to_rgb(cell["bg"])
            checked += 1
            if colour_distance(observed, expected) > BACKGROUND_TOLERANCE:
                failures.append(
                    f"row {row} col {cell['col']} background expected {cell['bg']} observed #{bytes(observed).hex()}"
                )
        if row == 2:
            observed = core_colour(
                shot, span(left, right), span(top, bottom), expected_fg
            )
            checked += 1
            if colour_distance(observed, expected_fg) > CORE_TOLERANCE:
                failures.append(
                    f"row {row} col {cell['col']} core expected {cell['fg']} observed #{bytes(observed).hex()}"
                )
    # Thin strokes at fractional positions may never cover one pixel completely, so the inverse word's core is
    # measured over the whole word rather than letter by letter.
    word = [cell for cell in cells if cell["row"] == 4]
    expected_core = hex_to_rgb(word[0]["fg"])
    observed = core_colour(
        shot,
        span(word[0]["left"], cell_box(word[-1])[2]),
        span(word[0]["top"], cell_box(word[0])[3]),
        expected_core,
    )
    checked += 1
    if colour_distance(observed, expected_core) > CORE_TOLERANCE:
        failures.append(
            f"row 4 core expected {word[0]['fg']} observed #{bytes(observed).hex()}"
        )
    if failures:
        return (
            False,
            f"{failures[0]} ({len(failures)} of {checked} colour measurements off)",
        )
    return (
        True,
        f"{checked} colour measurements: backgrounds exact, cores within {CORE_TOLERANCE}",
    )


def words_of(cells):
    """Row 5 cells grouped into words by contiguous columns."""
    words, current = [], []
    for cell in (c for c in cells if c["row"] == 5):
        if current and cell["col"] != current[-1]["col"] + 1:
            words.append(current)
            current = []
        current.append(cell)
    if current:
        words.append(current)
    return words


def ink_runs(shot, xs, y, background):
    longest = run = 0
    for x in xs:
        run = run + 1 if shot.is_ink(x, y, background) else 0
        longest = max(longest, run)
    return longest


def word_measurements(shot, word, font, theme_background, foreground):
    left, top, _, bottom = cell_box(word[0])
    right = cell_box(word[-1])[2]
    xs = span(left, right)
    baseline = word[0]["baseline"]
    height = bottom - top
    below = [
        y
        for y in span(top + (1 - UNDERLINE_BAND_FRACTION) * height, bottom)
        if y >= math.ceil(baseline)  # rows wholly below the baseline
    ]
    line_rows = {
        y
        for y in span(top, bottom)
        if ink_runs(shot, xs, y, theme_background) >= LINE_RUN_FRACTION * len(xs)
    }
    underline_run = any(y in line_rows for y in below)
    strike = any(
        y in line_rows
        for y in span(top + STRIKE_BAND[0] * height, top + STRIKE_BAND[1] * height)
    )
    centres, covered = [], 0
    for x in xs:
        weights = [(y, shot.difference(x, y, theme_background)) for y in below]
        weights = [(y, w) for y, w in weights if w > INK_THRESHOLD]
        if weights:
            covered += 1
            centres.append(sum(y * w for y, w in weights) / sum(w for _, w in weights))
    wave = max(centres) - min(centres) if centres else 0.0
    undercurl = (
        covered >= UNDERCURL_COLUMN_FRACTION * len(xs) and wave >= UNDERCURL_MIN_WAVE_PX
    )
    underline = underline_run and not undercurl
    # Rows with any part above the baseline: up to the row holding it, or the row above a whole-pixel baseline.
    letter_rows = [y for y in span(top, math.ceil(baseline)) if y not in line_rows]
    ink_count = sum(
        1 for x in xs for y in letter_rows if shot.is_ink(x, y, theme_background)
    )
    return {
        "shift": italic_shift(shot, xs, letter_rows, theme_background),
        "xs": xs,
        "letter_rows": letter_rows,
        "underline": underline,
        "undercurl": undercurl,
        "strikethrough": strike,
        "ink": ink_count,
        "core": core_colour(shot, xs, span(top, bottom), foreground),
        "left": left,
        "top": top,
        "baseline": baseline,
        "wave": wave,
    }


def regular_reference(word, font, theme_background, foreground):
    """The same letters drawn regular at this size, measured the way the screenshot's word is: ink count and slant."""
    left, top = cell_box(word[0])[0], word[0]["top"]
    right, bottom = cell_box(word[-1])[2], cell_box(word[0])[3]
    origin_x, origin_y = math.floor(left) - 2, math.floor(top) - 2
    width, height = math.ceil(right) - origin_x + 4, math.ceil(bottom) - origin_y + 4
    factor = REFERENCE_SUPERSAMPLE
    big = Image.new("RGB", (width * factor, height * factor), theme_background)
    big_font = ImageFont.truetype(str(FONT_REGULAR), font.size * factor)
    draw = ImageDraw.Draw(big)
    for cell in word:
        position = (
            (cell["left"] - origin_x) * factor,
            (word[0]["baseline"] - origin_y) * factor,
        )
        draw.text(position, cell["char"], font=big_font, fill=foreground, anchor="ls")
    shot = Shot(big.resize((width, height), Image.BOX), theme_background)
    xs = span(left - origin_x, right - origin_x)
    letter_rows = list(range(math.ceil(word[0]["baseline"]) - origin_y))
    ink = sum(1 for x in xs for y in letter_rows if shot.is_ink(x, y, theme_background))
    return ink, italic_shift(shot, xs, letter_rows, theme_background)


def italic_shift(shot, xs, letter_rows, theme_background):
    """Horizontal ink centre of the top quarter of the word's ink rows minus that of the bottom quarter."""
    rows = [
        y for y in letter_rows if any(shot.is_ink(x, y, theme_background) for x in xs)
    ]
    if len(rows) < 4:
        return 0.0
    quarter = max(len(rows) // 4, 1)

    def centre(selected):
        weights = [
            (x, shot.difference(x, y, theme_background)) for y in selected for x in xs
        ]
        weights = [(x, w) for x, w in weights if w > INK_THRESHOLD]
        return sum(x * w for x, w in weights) / sum(w for _, w in weights)

    return centre(rows[:quarter]) - centre(rows[-quarter:])


def check_attributes(shot, cells, font, scale, theme_background):
    foreground = hex_to_rgb(next(c for c in cells if c["row"] == 0)["fg"])
    problems, summary = [], []
    for word in words_of(cells):
        name = "".join(c["char"] for c in word)
        expected = set(word[0]["attrs"])
        measured = word_measurements(shot, word, font, theme_background, foreground)
        regular_ink, regular_shift = regular_reference(
            word, font, theme_background, foreground
        )
        ratio = measured["ink"] / max(regular_ink, 1)
        shift = measured["shift"] - regular_shift
        found = {
            "underline": measured["underline"],
            "undercurl": measured["undercurl"],
            "strikethrough": measured["strikethrough"],
            "bold": ratio >= BOLD_MIN_RATIO,
            "italic": shift >= ITALIC_MIN_SHIFT_PX,
            "dim": luminance(measured["core"])
            <= DIM_MAX_LUMINANCE_RATIO * luminance(foreground),
        }
        if "bold" not in expected and ratio > BOLD_ABSENT_MAX_RATIO:
            found["bold"] = True
        for attribute in ATTRIBUTE_WORDS:
            if found[attribute] != (attribute in expected):
                observed = "present" if found[attribute] else "absent"
                wanted = "present" if attribute in expected else "absent"
                problems.append(
                    f"{attribute} on {name} expected {wanted} observed {observed}"
                )
        summary.append(
            f"{name}: bold x{ratio:.2f} slant {shift:.2f} wave {measured['wave']:.2f}"
        )
    if problems:
        return False, f"{problems[0]} ({len(problems)} attribute mismatches)"
    return True, "; ".join(summary)


def ink_pixels(shot, xs, ys, background):
    return [(x, y) for x in xs for y in ys if shot.is_ink(x, y, background)]


def check_wide(shot, cells, theme_background):
    problems = []
    for cell in cells:
        left, top, right, bottom = cell_box(cell)
        xs, ys = span(left, right), span(top, bottom)
        if cell["row"] == 6:
            middle = (left + right) / 2
            for half, bounds in (("left", (left, middle)), ("right", (middle, right))):
                if (
                    len(ink_pixels(shot, span(*bounds), ys, theme_background))
                    < MIN_INK_PIXELS
                ):
                    problems.append(f"wide {cell['char']} {half} cell has no ink")
        elif cell["row"] in (7, 8, 9) and cell["char"] in BOX_EDGES:
            edge_pixels = {
                "l": (xs[0:1], ys),
                "r": (xs[-1:], ys),
                "t": (xs, ys[0:1]),
                "b": (xs, ys[-1:]),
            }
            for edge in BOX_EDGES[cell["char"]]:
                if not ink_pixels(shot, *edge_pixels[edge], theme_background):
                    problems.append(
                        f"box {cell['char']} row {cell['row']} col {cell['col']} line misses the {edge} edge"
                    )
        elif cell["row"] == 10:
            problems.extend(check_block(shot, cell, xs, ys, theme_background))
    shades = {
        c["char"]: shade_strength(shot, c, theme_background)
        for c in cells
        if c["row"] == 10 and c["char"] in "░▒▓"
    }
    if not shades["░"] < shades["▒"] < shades["▓"]:
        problems.append(f"shades ░ ▒ ▓ not ordered light to dark: {shades}")
    if problems:
        return False, f"{problems[0]} ({len(problems)} problems)"
    return (
        True,
        "wide characters inked, box lines reach cell edges, block elements fill their share",
    )


def shade_strength(shot, cell, theme_background):
    left, top, right, bottom = cell_box(cell)
    values = sorted(
        shot.difference(x, y, theme_background)
        for x in span(left, right)
        for y in span(top, bottom)
    )
    return values[len(values) // 2]


def check_block(shot, cell, xs, ys, background):
    char, ink = cell["char"], ink_pixels(shot, xs, ys, background)
    if char == "█":
        return (
            []
            if len(ink) == len(xs) * len(ys)
            else [f"█ fills {len(ink)} of {len(xs) * len(ys)} pixels"]
        )
    # A neighbour's anti-aliased edge blends into the border pixels of a cell, so half blocks are measured one pixel in.
    inner_xs, inner_ys = xs[1:-1], ys[1:-1]
    columns = {x for x, _ in ink_pixels(shot, xs, inner_ys, background)}
    rows = {y for _, y in ink_pixels(shot, inner_xs, ys, background)}
    half_rows, half_columns = len(ys) / 2, len(xs) / 2
    if char in "▀▄":
        count_ok = abs(len(rows) - half_rows) <= HALF_BLOCK_TOLERANCE_PX
        anchored = (ys[0] in rows) if char == "▀" else (ys[-1] in rows)
        return (
            []
            if count_ok
            and anchored
            and len({x for x, _ in ink} & set(inner_xs)) >= len(inner_xs)
            else [f"{char} fills {len(rows)} rows, expected {half_rows:.1f}"]
        )
    if char in "▌▐":
        count_ok = abs(len(columns) - half_columns) <= HALF_BLOCK_TOLERANCE_PX
        anchored = (xs[0] in columns) if char == "▌" else (xs[-1] in columns)
        return (
            []
            if count_ok
            and anchored
            and len({y for _, y in ink} & set(inner_ys)) >= len(inner_ys)
            else [f"{char} fills {len(columns)} columns, expected {half_columns:.1f}"]
        )
    return []


def cursor_result(shot, cell, shape, theme_background):
    left, top, right, bottom = cell_box(cell)
    xs, ys = span(left, right), span(top, bottom)
    ink = ink_pixels(shot, xs, ys, theme_background)
    if shape == "block":
        columns, rows = {x for x, _ in ink}, {y for _, y in ink}
        covers = (
            len(ink) >= BLOCK_COVERAGE_MIN * len(xs) * len(ys)
            and len(columns) >= len(xs) - CURSOR_BLOCK_TOLERANCE_PX
            and len(rows) >= len(ys) - CURSOR_BLOCK_TOLERANCE_PX
        )
        return covers, f"block covers {len(ink)} of {len(xs) * len(ys)} cell pixels"
    if shape == "bar":
        thickness = 0
        for x in xs:
            share = sum(1 for y in ys if shot.is_ink(x, y, theme_background)) / len(ys)
            if share < CURSOR_STRIP_ROW_FRACTION:
                break
            thickness += 1
        ok = CURSOR_STRIP_MIN_PX <= thickness <= CURSOR_STRIP_MAX_PX
        return ok, f"bar is {thickness} px wide"
    thickness = 0
    for y in reversed(ys):
        share = sum(1 for x in xs if shot.is_ink(x, y, theme_background)) / len(xs)
        if share < CURSOR_STRIP_ROW_FRACTION:
            break
        thickness += 1
    return (
        CURSOR_STRIP_MIN_PX <= thickness <= CURSOR_STRIP_MAX_PX,
        f"underline is {thickness} px high",
    )


def check_selection(shot, cells, argument, geometry):
    row, first, last, colour_text = argument.split(",")
    wanted = hex_to_rgb(colour_text)
    problems = []
    for column in range(int(first) - 1, int(last) + 2):
        left = geometry["origin"][0] + column * geometry["cell_width"]
        top = geometry["origin"][1] + int(row) * geometry["line_height"]
        right, bottom = left + geometry["cell_width"], top + geometry["line_height"]
        inner = [
            shot.get(x, y)
            for x in span(left + BOX_SHRINK_PX, right - BOX_SHRINK_PX)
            for y in span(top + BOX_SHRINK_PX, bottom - BOX_SHRINK_PX)
        ]
        near = colour_distance(dominant_colour(inner), wanted) <= SELECTION_TOLERANCE
        if near != (int(first) <= column <= int(last)):
            problems.append(
                f"column {column} {'has' if near else 'lacks'} the selection colour"
            )
    if problems:
        return False, f"{problems[0]} ({len(problems)} columns wrong)"
    return True, f"columns {first} to {last} selected, neighbours not"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("screenshot", type=Path)
    parser.add_argument("expected", type=Path)
    parser.add_argument(
        "--cursor", help="ROW,COL,SHAPE with SHAPE block, bar or underline"
    )
    parser.add_argument("--selection", help="ROW,COL_FROM,COL_TO,#rrggbb")
    arguments = parser.parse_args()
    expected = json.loads(arguments.expected.read_text(encoding="utf-8"))
    theme = json.loads(THEME_FILE.read_text(encoding="utf-8"))
    theme_background = hex_to_rgb(theme["colors"]["background"])
    shot = Shot(Image.open(arguments.screenshot), theme_background)
    cells, scale = expected["cells"], expected["scale"]
    font = Font(scale)
    results = [
        (
            "placement",
            *check_placement(
                shot,
                cells,
                font,
                theme_background,
                parse_selection(arguments.selection),
            ),
        ),
        ("colours", *check_colours(shot, cells, theme_background)),
        ("attributes", *check_attributes(shot, cells, font, scale, theme_background)),
        ("wide", *check_wide(shot, cells, theme_background)),
    ]
    if arguments.cursor:
        row, column, shape = arguments.cursor.split(",")
        geometry_cell = {
            "left": expected["origin"][0] + int(column) * expected["cell_width"],
            "top": expected["origin"][1] + int(row) * expected["line_height"],
            "width": expected["cell_width"],
            "height": expected["line_height"],
        }
        results.append(
            ("cursor", *cursor_result(shot, geometry_cell, shape, theme_background))
        )
    if arguments.selection:
        results.append(
            ("selection", *check_selection(shot, cells, arguments.selection, expected))
        )
    failed = False
    for name, passed, detail in results:
        print(f"ok: {name} {detail}" if passed else f"FAIL: {name} {detail}")
        failed = failed or not passed
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
