#!/usr/bin/env python3
"""Expected values for G2's design and controls gates, in device pixels, from the rulebook and the font files only.

Usage: python3 scripts/window-chrome/expected.py --scale S --theme graphite-dark > expected-S-graphite-dark.json

Reads rulebook-tokens.json (the rulebook's tables, transcribed once, each entry with its source line) and the bundled
Inter files. It must not read the token module or any code of the program under test. Each value carries its tolerance
and where the tolerance comes from. A value the rulebook does not give is left out and listed under "omitted".
"""

import argparse
import json
import math
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
FONT_FILES = {
    400: REPO_ROOT / "assets/fonts/Inter-Regular.ttf",
    500: REPO_ROOT / "assets/fonts/Inter-Medium.ttf",
    600: REPO_ROOT / "assets/fonts/Inter-SemiBold.ttf",
}
CAP_GLYPH = "H"
NO_TEXT_KINDS = ("switch", "icon-button")  # no label of their own (rulebook:227)


def load_tokens():
    return json.loads((SCRIPT_DIR / "rulebook-tokens.json").read_text(encoding="utf-8"))


def round_half_up(value):
    return math.floor(value + 0.5)


def hex_to_rgb(text):
    return tuple(int(text[i : i + 2], 16) for i in (1, 3, 5))


def rgb_to_hex(rgb):
    return "#" + "".join(f"{round_half_up(v):02x}" for v in rgb)


def blend(base, over, alpha):
    return tuple(b + (o - b) * alpha for b, o in zip(base, over))


def luminance(rgb):
    def linear(channel):
        c = channel / 255
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (linear(v) for v in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(first, second):
    high, low = sorted((luminance(first), luminance(second)), reverse=True)
    return (high + 0.05) / (low + 0.05)


def cap_height_px(weight, size_px):
    """The height of H at size_px: the glyph's yMax from the font file over units per em, times the size."""
    font = TTFont(FONT_FILES[weight])
    glyf = font["glyf"]
    glyph = glyf[font.getBestCmap()[ord(CAP_GLYPH)]]
    glyph.recalcBounds(glyf)
    if glyph.yMin != 0:
        raise ValueError(
            f"{FONT_FILES[weight].name}: {CAP_GLYPH} does not sit on the baseline (yMin {glyph.yMin})"
        )
    return glyph.yMax * size_px / font["head"].unitsPerEm


def measure(logical, scale, tolerance, src):
    """A size: logical px, device px (exact and rounded), the tolerance in device px and the sources."""
    device = logical * scale
    return {
        "logical": logical,
        "device": device,
        "device_rounded": round_half_up(device),
        "tolerance_px": tolerance["value"],
        "tolerance_src": tolerance["src"],
        "src": src,
    }


def tokens_focus_alpha():
    return load_tokens()["states"]["focus"]["ring_alpha"]


def theme_colours(theme):
    colours = {}
    for role in (
        "background",
        "card",
        "accent",
        "foreground",
        "muted_foreground",
        "err",
        "selection",
        "pressed",
        "ring",
        "primary",
        "primary_pressed",
        "primary_foreground",
    ):
        colours[role] = {"hex": theme[role]["hex"], "src": theme[role]["src"]}
    background = hex_to_rgb(theme["background"]["hex"])
    card = hex_to_rgb(theme["card"]["hex"])
    foreground = hex_to_rgb(theme["foreground"]["hex"])
    border = theme["border"]
    colours["border_over_background"] = {
        "hex": rgb_to_hex(
            blend(background, hex_to_rgb(border["over"]), border["alpha"])
        ),
        "rule": f"background with {border['over']} at {border['alpha']:.0%}",
        "src": border["src"],
    }
    colours["focus_ring_over_card"] = {
        "hex": rgb_to_hex(
            blend(
                card,
                hex_to_rgb(theme["ring"]["hex"]),
                tokens_focus_alpha(),
            )
        ),
        "rule": "card with ring at the focus ring's alpha",
        "src": theme["ring"]["src"],
    }
    colours["input"] = {
        "over": theme["input"]["over"],
        "alpha": theme["input"]["alpha"],
        "src": theme["input"]["src"],
    }
    colours["frame"] = {
        "hex": rgb_to_hex(
            blend(card, foreground, theme["frame"]["mix_card_toward_foreground"])
        ),
        "rule": "card moved toward foreground",
        "src": theme["frame"]["src"],
    }
    return colours


def selection_contrast(theme, tokens):
    selection = hex_to_rgb(theme["selection"]["hex"])
    limits = tokens["selection_contrast"]
    against_background = contrast(selection, hex_to_rgb(theme["background"]["hex"]))
    foreground_on_selection = contrast(
        hex_to_rgb(theme["foreground"]["hex"]), selection
    )
    return {
        "selection_against_background": {
            "ratio": round(against_background, 3),
            "at_least": limits["selection_against_background_min"]["ratio"],
            "src": limits["selection_against_background_min"]["src"],
        },
        "foreground_on_selection": {
            "ratio": round(foreground_on_selection, 3),
            "at_least": limits["foreground_on_selection_min"]["ratio"],
            "src": limits["foreground_on_selection_min"]["src"],
        },
    }


def cap_heights(tokens, scale):
    result = {}
    tolerance = tokens["tolerance"]["cap_height_px"]
    for role, text in tokens["text"].items():
        weight = text["weight"]
        logical = cap_height_px(weight, text["px"])
        result[role] = {
            "glyph": CAP_GLYPH,
            "font": FONT_FILES[weight].name,
            "size_px": text["px"],
            "logical": logical,
            "device": logical * scale,
            "tolerance_px": tolerance["value"],
            "tolerance_src": tolerance["src"],
            "src": text["src"],
        }
    return result


def chrome_values(tokens, scale):
    edge = tokens["tolerance"]["edge_px"]
    heights, pane = tokens["heights"], tokens["pane"]
    values = {
        "title_bar_height": measure(
            heights["h_title_bar"]["px"], scale, edge, heights["h_title_bar"]["src"]
        ),
        "tab_height": measure(
            heights["h_tab"]["px"], scale, edge, heights["h_tab"]["src"]
        ),
        "tab_radius": measure(
            tokens["radius"]["radius_md"]["px"],
            scale,
            edge,
            tokens["radius"]["radius_md"]["src"],
        ),
        "status_height": measure(
            heights["h_status"]["px"], scale, edge, heights["h_status"]["src"]
        ),
        "pane_radius": measure(
            pane["radius"]["px"], scale, edge, pane["radius"]["src"]
        ),
        "pane_gap": measure(pane["gap"]["px"], scale, edge, pane["gap"]["src"]),
        "pane_padding": measure(
            pane["padding"]["px"], scale, edge, pane["padding"]["src"]
        ),
        "pane_margin": measure(
            pane["margin"]["px"], scale, edge, pane["margin"]["src"]
        ),
    }
    colour_tolerance = tokens["tolerance"]["colour_levels"]
    values["pane_border_alpha"] = {
        "value": pane["border_alpha"]["value"],
        "tolerance_levels": colour_tolerance["value"],
        "tolerance_src": colour_tolerance["src"],
        "src": pane["border_alpha"]["src"],
    }
    values["hairline_device_px"] = {
        "device": tokens["hairline"]["device_px"],
        "every_scale": tokens["hairline"]["every_scale"],
        "tolerance_px": 0,
        "tolerance_src": "a border is exactly one whole device pixel (rulebook:160-161)",
        "src": tokens["hairline"]["src"],
    }
    ring = tokens["states"]["focus"]
    values["focus_ring_width"] = measure(
        ring["ring_width_px"], scale, edge, ring["src"]
    )
    return values


def control_values(tokens, scale):
    edge = tokens["tolerance"]["edge_px"]
    centring = tokens["tolerance"]["centring_px"]
    result, omitted = {}, []
    for kind, control in tokens["controls"].items():
        entry = {}
        if "height" in control:
            entry["height"] = measure(
                control["height"]["px"], scale, edge, control["height"]["src"]
            )
        else:
            omitted.append(f"{kind}: height")
        for extra in ("height_in_rows", "width"):
            if extra in control:
                entry[extra] = measure(
                    control[extra]["px"], scale, edge, control[extra]["src"]
                )
        if "radius" in control:
            entry["radius"] = measure(
                control["radius"]["px"], scale, edge, control["radius"]["src"]
            )
        elif control.get("radius_is_half_height"):
            entry["radius"] = measure(
                control["height"]["px"] / 2, scale, edge, control["radius_src"]
            )
        else:
            omitted.append(f"{kind}: radius")
        if "text" in control:
            text = tokens["text"][control["text"]]
            entry["text_role"] = control["text"]
            entry["text_px"] = text["px"]
            entry["text_weight"] = text["weight"]
            entry["text_src"] = control["text_src"]
        elif kind not in NO_TEXT_KINDS:
            omitted.append(f"{kind}: text size")
        entry["icon"] = measure(
            control["icon_px"], scale, centring, control["icon_src"]
        )
        entry["centring_tolerance_px"] = centring["value"]
        entry["centring_tolerance_src"] = centring["src"]
        result[kind] = entry
    return result, omitted


def resolve_looks(looks, theme):
    """The looks with every colour role named replaced by that role's hex in this theme, where the rulebook gives one."""
    roles = {role: entry["hex"] for role, entry in theme.items() if "hex" in entry}

    def resolve(value):
        if isinstance(value, dict):
            return {key: resolve(item) for key, item in value.items()}
        if isinstance(value, str) and value in roles:
            return {"role": value, "hex": roles[value]}
        return value

    return resolve(looks)


def build(scale, theme_name):
    tokens = load_tokens()
    theme = tokens["themes"][theme_name]
    controls, omitted = control_values(tokens, scale)
    return {
        "scale": scale,
        "theme": theme_name,
        "chrome": chrome_values(tokens, scale),
        "controls": controls,
        "cap_heights": cap_heights(tokens, scale),
        "colours": theme_colours(theme),
        "selection_contrast": selection_contrast(theme, tokens),
        "states": tokens["states"],
        "looks": resolve_looks(tokens["looks"], theme),
        "colour_tolerance_levels": tokens["tolerance"]["colour_levels"],
        "omitted": omitted + [f"{gap}" for gap in tokens["gaps"]],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--scale",
        type=float,
        required=True,
        help="device pixels per logical pixel, such as 1 or 1.5",
    )
    parser.add_argument(
        "--theme", required=True, choices=sorted(load_tokens()["themes"])
    )
    arguments = parser.parse_args()
    if arguments.scale <= 0:
        sys.exit("expected.py: --scale must be greater than zero")
    json.dump(build(arguments.scale, arguments.theme), sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
