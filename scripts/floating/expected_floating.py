#!/usr/bin/env python3
"""Expected values for G3's floating screens, in device pixels, from the rulebook and the font files only.

Usage: python3 scripts/floating/expected_floating.py --scale S --theme graphite-dark > expected-floating-S-graphite-dark.json

It extends window-chrome/expected.py (the chrome, controls, cap heights and colours) with the popover rows, the
popover edge and its shadow profile. Nothing here reads the program under test.

The shadow: a box shadow `0 12 32` in black at 35 %. CSS defines the blur radius as twice the standard deviation, so
sigma is 16 logical px. A Gaussian blur of a sharp rectangle is separable: the share of the shadow at a point is the
product of the horizontal and the vertical shares, each a difference of two normal distribution functions at the
shadow rectangle's edges. The expected grey at a point is the background under the popover times (1 - 0.35 x share).
"""

import argparse
import json
import math
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR.parent / "window-chrome"))
import expected as chrome  # noqa: E402

SHADOW_SAMPLE_STEP_DEVICE_PX = 2
SHADOW_TOLERANCE_LEVELS = 3  # the same as the colour tolerance of the chrome gates


def normal_cdf(value):
    return 0.5 * (1 + math.erf(value / math.sqrt(2)))


def shadow_share(position, low, high, sigma):
    """The share of a Gaussian-blurred segment [low, high] that reaches `position`, on one axis."""
    return normal_cdf((position - low) / sigma) - normal_cdf((position - high) / sigma)


def shadow_alpha(point, box, shadow, scale):
    """The black the shadow lays over `point` (device px) for a popover with `box` [x, y, w, h] (device px)."""
    sigma = shadow["blur"] / 2 * scale
    x0, y0 = box[0], box[1] + shadow["offset_y"] * scale
    x1, y1 = box[0] + box[2], y0 + box[3]
    horizontal = shadow_share(point[0], x0, x1, sigma)
    vertical = shadow_share(point[1], y0, y1, sigma)
    return shadow["black_alpha"] * horizontal * vertical


def shadow_profile(box, background_grey, shadow, scale):
    """Expected greys at the pixels below the popover's bottom edge, along its middle column."""
    middle = box[0] + box[2] // 2
    reach = int(shadow["offset_y"] * scale + 3 * shadow["blur"] / 2 * scale)
    profile = []
    for below in range(0, reach, SHADOW_SAMPLE_STEP_DEVICE_PX):
        y = box[1] + box[3] + below
        alpha = shadow_alpha((middle + 0.5, y + 0.5), box, shadow, scale)
        profile.append(
            {"below_device_px": below, "grey": background_grey * (1 - alpha)}
        )
    return profile


def floating_values(tokens, scale):
    edge = tokens["tolerance"]["edge_px"]
    result = {}
    for name, entry in tokens["floating"].items():
        if "px" in entry:
            result[name] = chrome.measure(entry["px"], scale, edge, entry["src"])
        else:
            result[name] = {
                "device": entry["device_px"],
                "tolerance_px": 0,
                "tolerance_src": "a border is exactly one whole device pixel (rulebook:160-161)",
                "src": entry["src"],
            }
    return result


def build(scale, theme):
    tokens = chrome.load_tokens()
    values = chrome.build(scale, theme)
    values["floating"] = floating_values(tokens, scale)
    shadow = tokens["shadows"]["popover"]
    values["popover_shadow"] = {
        "offset_y": shadow["offset_y"],
        "blur": shadow["blur"],
        "sigma_logical": shadow["blur"] / 2,
        "black_alpha": shadow["black_alpha"],
        "tolerance_levels": SHADOW_TOLERANCE_LEVELS,
        "model": "Gaussian of a sharp rectangle, separable; see this file's docstring",
        "src": shadow["src"],
    }
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--scale", type=float, required=True)
    parser.add_argument(
        "--theme", required=True, choices=sorted(chrome.load_tokens()["themes"])
    )
    arguments = parser.parse_args()
    if arguments.scale <= 0:
        sys.exit("expected_floating.py: --scale must be greater than zero")
    json.dump(build(arguments.scale, arguments.theme), sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
