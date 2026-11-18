#!/usr/bin/env python3
"""Contrast and family checks for the Neutral and Zinc themes (and any theme file given on the command line).

    scripts/themes/check_contrast.py [THEME.json ...]     check the four Neutral and Zinc files, or the ones named
    scripts/themes/check_contrast.py --faults             plant one wrong role per run; each must be caught

Every pair is printed with its ratio and the least it may be. The ratios are WCAG 2.x contrast, worked out here from
the files' own colours (translucent colours are laid over the surface they sit on). Exit 1 when a pair is short or a
family rule is broken. The limits, in the order they are checked:

  text            foreground on every surface and on the selection          4.5
  secondary text  muted_foreground on background, card and pressed           4.5
  primary button  primary_foreground on primary and on primary_pressed       4.5
  hover fill      accent against background                                 1.2
  focus ring      ring against background and card                           3.0
  selection       selection against background                              1.5
  status colours  ok, err, agent on every surface, and over their 10 % tint  4.5
  terminal        ANSI 1-6 and 9-14 on background                            3.0
  family          Neutral: every grey is pure grey; Zinc: every grey leans blue (blue above red, at most 12 steps)
"""

import json
import sys
from pathlib import Path
from typing import Any

Theme = dict[str, Any]

THEME_DIR = Path(__file__).resolve().parent.parent.parent / "assets" / "themes"
FILES = ("neutral-dark", "neutral-light", "zinc-dark", "zinc-light")
SURFACES = (
    "background",
    "sidebar",
    "card",
    "muted",
    "accent",
    "pressed",
    "sidebar_accent",
)
STATUS_SURFACES = ("background", "card", "muted", "accent", "pressed")
TINT = 0.1
GREYS = (
    "sidebar",
    "background",
    "card",
    "muted",
    "foreground",
    "muted_foreground",
    "accent",
    "pressed",
    "ring",
    "primary",
    "primary_foreground",
    "sidebar_accent",
    "robot_paint",
    "primary_pressed",
    "selection",
)
GREY_ANSI = (0, 7, 8, 15)
MAX_LEAN = 12
NEAR_WHITE = 0.9  # relative luminance above which a zinc grey may be pure (the scale's own 50 step is #fafafa)


def parse(text: str) -> list[float]:
    """RGBA floats 0..1 from #rrggbb or #rrggbbaa."""
    digits = text.removeprefix("#")
    if len(digits) not in (6, 8):
        raise ValueError(f"not a colour: {text!r}")
    channels = [int(digits[i : i + 2], 16) / 255 for i in range(0, len(digits), 2)]
    return channels + [1.0] * (4 - len(channels))


def flatten(colour: list[float], surface: list[float]) -> list[float]:
    return [colour[i] * colour[3] + surface[i] * (1 - colour[3]) for i in range(3)]


def luminance(rgb: list[float]) -> float:
    def linear(value: float) -> float:
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    return sum(
        weight * linear(value)
        for weight, value in zip((0.2126, 0.7152, 0.0722), rgb, strict=True)
    )


def ratio(top: list[float], under: list[float]) -> float:
    """Contrast of one colour laid over a surface (a colour with alpha is flattened onto it first)."""
    a, b = sorted((luminance(flatten(top, under)), luminance(under[:3])), reverse=True)
    return (a + 0.05) / (b + 0.05)


def pairs(theme: Theme) -> list[tuple[str, float, float]]:
    """Every (label, ratio, least) for one theme document."""
    c = {name: parse(value) for name, value in theme["colors"].items()}
    c["selection"] = parse(theme["selection"])
    ansi = [parse(value) for value in theme["ansi"]]
    out = []
    for surface in (*SURFACES, "selection"):
        under = c[surface]
        out.append((f"foreground on {surface}", ratio(c["foreground"], under), 4.5))
    for surface in ("background", "card", "pressed"):
        out.append(
            (
                f"muted_foreground on {surface}",
                ratio(c["muted_foreground"], c[surface]),
                4.5,
            )
        )
    for button in ("primary", "primary_pressed"):
        out.append(
            (
                f"primary_foreground on {button}",
                ratio(c["primary_foreground"], c[button]),
                4.5,
            )
        )
    out.append(
        (
            "hover fill (accent) against background",
            ratio(c["accent"], c["background"]),
            1.2,
        )
    )
    for surface in ("background", "card"):
        out.append((f"ring against {surface}", ratio(c["ring"], c[surface]), 3.0))
    out.append(
        ("selection against background", ratio(c["selection"], c["background"]), 1.5)
    )
    for token in ("ok", "err", "agent"):
        for surface in STATUS_SURFACES:
            out.append((f"{token} on {surface}", ratio(c[token], c[surface]), 4.5))
        tint = [*c[token][:3], TINT]
        badge = [*flatten(tint, c["muted"]), 1.0]
        out.append((f"{token} on its 10% tint over muted", ratio(c[token], badge), 4.5))
    for index in (*range(1, 7), *range(9, 15)):
        out.append(
            (f"ansi {index} on background", ratio(ansi[index], c["background"]), 3.0)
        )
    return out


def family_problems(theme: Theme, family: str) -> list[str]:
    """Strings naming each grey that breaks the family rule; none when the family holds."""
    greys = {
        name: parse(theme["colors"][name]) for name in GREYS if name in theme["colors"]
    }
    greys["selection"] = parse(theme["selection"])
    for index in GREY_ANSI:
        greys[f"ansi {index}"] = parse(theme["ansi"][index])
    problems = []
    for name, colour in greys.items():
        red, green, blue = (round(x * 255) for x in colour[:3])
        if family == "neutral" and not red == green == blue:
            problems.append(
                f"{name} is #{red:02x}{green:02x}{blue:02x}, not a pure grey"
            )
        if family == "zinc":
            bright = luminance(colour[:3]) > NEAR_WHITE
            if (
                red != green
                or blue < red
                or blue - red > MAX_LEAN
                or (blue == red and not bright)
            ):
                problems.append(
                    f"{name} is #{red:02x}{green:02x}{blue:02x}, which does not lean blue by 1 to {MAX_LEAN}"
                )
    return problems


def check(theme: Theme, label: str, quiet: bool = False) -> int:
    """Print and count the misses of one theme."""
    misses = 0
    family = label.split("-")[0]
    for what, got, least in pairs(theme):
        ok = got >= least - 1e-9
        misses += not ok
        if not quiet or not ok:
            print(
                f"{'ok  ' if ok else 'MISS'} {label}: {what}: {got:.2f}:1 (at least {least})"
            )
    for problem in family_problems(theme, family):
        misses += 1
        print(f"MISS {label}: family: {problem}")
    return misses


def load(label: str) -> Theme:
    theme: Theme = json.loads((THEME_DIR / f"{label}.json").read_text())
    return theme


def planted(theme: Theme, where: str, key: str | int | None, value: str) -> Theme:
    """A copy of the theme with one role made wrong. `where` is "colors", "ansi" or "selection"."""
    wrong: Theme = json.loads(json.dumps(theme))
    if where == "selection":
        wrong["selection"] = value
    else:
        wrong[where][key] = value
    return wrong


# (theme, where, key, wrong value, what is wrong)
FAULTS = (
    ("neutral-dark", "colors", "muted_foreground", "#4a4a4a", "secondary text too dim"),
    ("neutral-light", "colors", "ring", "#e0e0e0", "focus ring too faint"),
    ("zinc-dark", "selection", None, "#101013", "selection too close to the page"),
    ("zinc-light", "colors", "err", "#e8a0a0", "err too pale"),
    ("zinc-dark", "ansi", 4, "#101018", "ansi 4 lost on the background"),
    ("neutral-dark", "colors", "card", "#171718", "card off pure grey"),
    (
        "zinc-dark",
        "colors",
        "background",
        "#0a0a0a",
        "family drift: a zinc page with no blue lean",
    ),
)


def run_faults() -> int:
    uncaught = 0
    for label, where, key, value, what in FAULTS:
        wrong = planted(load(label), where, key, value)
        print(f"-- planted: {label}: {what}")
        if check(wrong, label, quiet=True) == 0:
            print(f"NOT CAUGHT: {label}: {what}")
            uncaught += 1
    print("faults: all caught" if not uncaught else f"faults: {uncaught} not caught")
    return 1 if uncaught else 0


def main(argv: list[str]) -> int:
    if argv == ["--faults"]:
        if check_all(quiet=True):
            print("the real files must pass before faults mean anything")
            return 1
        return run_faults()
    if argv:
        labels = [Path(a).stem for a in argv]
        documents = [json.loads(Path(a).read_text()) for a in argv]
    else:
        labels, documents = list(FILES), [load(name) for name in FILES]
    misses = sum(
        check(doc, label) for doc, label in zip(documents, labels, strict=True)
    )
    print(f"{'FAIL' if misses else 'PASS'}: {len(documents)} themes, {misses} misses")
    return 1 if misses else 0


def check_all(quiet: bool) -> int:
    return sum(check(load(name), name, quiet) for name in FILES)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
