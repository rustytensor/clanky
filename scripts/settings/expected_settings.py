#!/usr/bin/env python3
"""Expected values for G4's Settings window, in device pixels, from the rulebook and the font files only.

Usage: python3 scripts/settings/expected_settings.py --scale S --theme graphite-dark > expected-settings-S.json

It extends floating/expected_floating.py (itself window-chrome/expected.py) with the Settings window's sizes (the `settings` section of
rulebook-tokens.json: the panel, header, section switcher, column, paddings, cards and gaps) and with where the first
card's title and the first group label start, worked out from the Inter files' own metrics: the line box puts a
glyph's top (ascender minus the glyph's top) below the box's top, and a glyph's ink starts its left side bearing in.
Nothing here reads the program under test.
"""

import argparse
import json
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR.parent / "window-chrome"))
sys.path.insert(0, str(SCRIPT_DIR.parent / "floating"))
import expected as chrome
import expected_floating

BORDER_WIDTH_DEVICE_PX = 1  # a border is one whole device pixel (rulebook amendments)


def glyph_offsets(weight, char, px):
    """(left side bearing, distance from the font's ascender down to the glyph's top) of a glyph, in px."""
    font = TTFont(str(chrome.FONT_FILES[weight]))
    glyf = font["glyf"]
    glyph = glyf[font.getBestCmap()[ord(char)]]
    glyph.recalcBounds(glyf)
    em = font["head"].unitsPerEm
    return glyph.xMin * px / em, (font["hhea"].ascent - glyph.yMax) * px / em


def settings_values(tokens, scale):
    edge = tokens["tolerance"]["edge_px"]
    values = {}
    for name, entry in tokens["settings"].items():
        if "px" in entry:
            values[name] = chrome.measure(entry["px"], scale, edge, entry["src"])
        else:
            values[name] = {"value": entry["value"], "src": entry["src"]}
    return values


def offsets(tokens, scale):
    """Where the first card's title ink and the first group label ink start, in device px."""
    text = tokens["text"]
    settings = tokens["settings"]
    title = text["text_title"]
    group = text["text_ui"]
    bearing, drop = glyph_offsets(title["weight"], "T", title["px"] * scale)
    _, label_drop = glyph_offsets(500, "L", group["px"] * scale)
    border = BORDER_WIDTH_DEVICE_PX
    return {
        "card_padding_x": {
            "device": border + settings["card_padding_x"]["px"] * scale + bearing,
            "rule": "1 + 12 x scale + the left side bearing of T (Inter Medium, 12.5 px)",
        },
        "card_padding_y": {
            "device": border + settings["card_padding_y"]["px"] * scale + drop,
            "rule": "1 + 10 x scale + (ascender - top of T) at 12.5 px",
        },
        "top_padding": {
            "device": settings["padding_top"]["px"] * scale + label_drop,
            "rule": "24 x scale + (ascender - top of L) of the first group label (Inter Medium, 12 px)",
        },
    }


def card_border_share(tokens, theme):
    """The card's border as a share of the theme's border colour laid over its fill: 60 % of the 9 % border."""
    border = tokens["themes"][theme]["border"]["alpha"]
    return border * tokens["settings"]["card_border_opacity"]["value"]


def card_border_over(tokens, theme):
    """The colour the border is laid over the fill with: white on a dark theme, black on a light one."""
    hex_colour = tokens["themes"][theme]["border"]["over"]
    return tuple(int(hex_colour[i : i + 2], 16) for i in (1, 3, 5))


def build(scale, theme):
    tokens = chrome.load_tokens()
    values = expected_floating.build(
        scale, theme
    )  # the chrome's values and the floating shadow, the panel's too
    values["settings"] = settings_values(tokens, scale)
    values["settings_offsets"] = offsets(tokens, scale)
    values["settings_card_border_share"] = card_border_share(tokens, theme)
    values["settings_card_border_over"] = card_border_over(tokens, theme)
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--scale", type=float, required=True)
    parser.add_argument("--theme", default="graphite-dark")
    arguments = parser.parse_args()
    json.dump(build(arguments.scale, arguments.theme), sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
