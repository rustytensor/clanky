#!/usr/bin/env python3
"""Prints every value where rulebook-tokens.json and the design-tokens branch disagree. It reports; it fixes neither side.

Usage: python3 scripts/window-chrome/check_tokens_match.py [--branch design-tokens]

Reads crates/clanky/src/calm/tokens.rs and assets/themes/graphite-{dark,light}.json from the branch with `git show`.
Lines are "DIFFERENT", "ONLY IN TOKENS" (the token module has a value the rulebook table does not) or "ONLY IN
RULEBOOK". Exit status is always 0: this is information for the lead.
"""

import argparse
import json
import re
import subprocess
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
TOKENS_PATH = "crates/clanky/src/calm/tokens.rs"
THEME_PATHS = {
    "graphite-dark": "assets/themes/graphite-dark.json",
    "graphite-light": "assets/themes/graphite-light.json",
}
CONSTANT = re.compile(r"^pub const (\w+): (?:f32|u16|u32) = ([^;]+);", re.MULTILINE)
ARITHMETIC = re.compile(r"^[\w\s.+\-*/()]+$")
OPACITY_TOLERANCE = 0.0005


def git_show(branch, path):
    result = subprocess.run(
        ["git", "show", f"{branch}:{path}"],
        capture_output=True,
        text=True,
        check=False,
        cwd=REPO_ROOT,
        timeout=60,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"check_tokens_match.py: git show {branch}:{path} failed: {result.stderr.strip()}"
        )
    return result.stdout


def parse_constants(source):
    """Every `pub const NAME: f32 = expression;` with simple arithmetic and earlier constants resolved."""
    values = {}
    for name, expression in CONSTANT.findall(source):
        text = re.sub(r"(?<=\d)_?f32\b|(?<=\d)_(?=\d)", "", expression)
        if not ARITHMETIC.match(text):
            continue
        try:
            values[name] = eval(text, {"__builtins__": {}}, values)  # noqa: S307 - arithmetic on checked characters
        except NameError:
            continue
    return values


def rulebook_pairs(book):
    """(label, rulebook value and source, token constant) for every rulebook number the token module should hold."""
    pairs = []
    for role, text in book["text"].items():
        pairs.append(
            (
                f"text {role} px",
                text["px"],
                text["src"],
                "TEXT_"
                + {
                    "text_tree": "TREE",
                    "text_title": "TITLE",
                    "text_ui": "UI",
                    "text_tab_switch": "TAB_SWITCH",
                    "text_status": "STATUS",
                    "text_desc": "DESCRIPTION",
                    "text_meta": "META",
                }[role],
            )
        )
    for role, name in (
        ("radius_sm", "SMALL"),
        ("radius_md", "MEDIUM"),
        ("radius_lg", "LARGE"),
        ("radius_xl", "PANE"),
        ("window", "WINDOW"),
    ):
        pairs.append(
            (
                f"radius {role}",
                book["radius"][role]["px"],
                book["radius"][role]["src"],
                "RADIUS_" + name,
            )
        )
    for index, step in enumerate(book["spacing_steps"]["px"]):
        pairs.append(
            (
                f"spacing step {index + 1}",
                step,
                book["spacing_steps"]["src"],
                f"SPACE_{step}",
            )
        )
    heights = {
        "h_title_bar": "TITLE_BAR_HEIGHT",
        "h_tab": "TAB_HEIGHT",
        "h_row": "ROW_HEIGHT",
        "h_menu_item": "MENU_ITEM_HEIGHT",
        "h_palette_row": "PALETTE_ROW_HEIGHT",
        "h_palette_input": "PALETTE_INPUT_HEIGHT",
        "h_status": "STATUS_HEIGHT",
        "h_button_small": "BUTTON_HEIGHT_SMALL",
        "h_button_bar": "BUTTON_HEIGHT_BAR",
        "h_button_default": "BUTTON_HEIGHT_DEFAULT",
        "h_button_large": "BUTTON_HEIGHT_LARGE",
        "h_chip": "CHIP_HEIGHT",
        "h_block_bar": "BLOCK_TOOLBAR_HEIGHT",
        "h_dialog_header": "DIALOG_HEADER_HEIGHT",
    }
    for role, name in heights.items():
        pairs.append(
            (
                f"height {role}",
                book["heights"][role]["px"],
                book["heights"][role]["src"],
                name,
            )
        )
    controls = book["controls"]
    pairs += [
        (
            "field height",
            controls["field"]["height"]["px"],
            controls["field"]["height"]["src"],
            "FIELD_HEIGHT",
        ),
        (
            "switch height",
            controls["switch"]["height"]["px"],
            controls["switch"]["height"]["src"],
            "SWITCH_CONTROL_HEIGHT",
        ),
        (
            "switch width",
            controls["switch"]["width"]["px"],
            controls["switch"]["width"]["src"],
            "SWITCH_CONTROL_WIDTH",
        ),
        (
            "segment height",
            controls["segment"]["height"]["px"],
            controls["segment"]["height"]["src"],
            "SEGMENT_HEIGHT",
        ),
    ]
    pane = book["pane"]
    pairs += [
        ("pane gap", pane["gap"]["px"], pane["gap"]["src"], "PANE_GAP"),
        ("pane padding", pane["padding"]["px"], pane["padding"]["src"], "PANE_PADDING"),
        # The token module has one constant for the gap between panes and the window's edge too.
        ("pane margin", pane["margin"]["px"], pane["margin"]["src"], "PANE_GAP"),
        (
            "side column width",
            pane["side_column_width"]["px"],
            pane["side_column_width"]["src"],
            "SIDE_COLUMN_WIDTH",
        ),
        (
            "border width",
            pane["border_width"]["device_px"],
            pane["border_width"]["src"],
            "BORDER_WIDTH",
        ),
        (
            "palette width",
            book["palette"]["width"]["px"],
            book["palette"]["width"]["src"],
            "PALETTE_WIDTH",
        ),
        (
            "block toolbar button width",
            book["block"]["toolbar_button_width"]["px"],
            book["block"]["toolbar_button_width"]["src"],
            "BLOCK_TOOLBAR_BUTTON_WIDTH",
        ),
        (
            "icon size in rows",
            book["icons"]["row_px"],
            book["icons"]["src"],
            "ICON_SIZE_ROW",
        ),
        (
            "icon size in bars",
            book["icons"]["bar_px"],
            book["icons"]["src"],
            "ICON_SIZE_BAR",
        ),
        (
            "icon stroke",
            book["icons"]["stroke_px"],
            book["icons"]["src"],
            "ICON_STROKE_WIDTH",
        ),
        (
            "focus ring width",
            book["states"]["focus"]["ring_width_px"],
            book["states"]["focus"]["src"],
            "FOCUS_RING_WIDTH",
        ),
        (
            "hit box minimum",
            book["geometry_rules"]["hit_box_min_px"]["value"],
            book["geometry_rules"]["hit_box_min_px"]["src"],
            "MIN_HIT_SIZE",
        ),
    ]
    opacities = (
        ("border alpha", book["pane"]["border_alpha"], "BORDER_OPACITY"),
        (
            "active tab fill",
            book["active_tab"]["fill_foreground_alpha"],
            "ACTIVE_TAB_FILL",
        ),
        (
            "active tab inner ring",
            book["active_tab"]["inner_ring_foreground_alpha"],
            "ACTIVE_TAB_RING",
        ),
        (
            "block toolbar rest opacity",
            book["block"]["toolbar_rest_opacity"],
            "BLOCK_TOOLBAR_REST_OPACITY",
        ),
        ("block divider", book["block"]["divider_foreground_alpha"], "BLOCK_DIVIDER"),
        (
            "block divider after failure",
            book["block"]["divider_failed_err_alpha"],
            "BLOCK_DIVIDER_FAILED",
        ),
        (
            "focus ring alpha",
            {
                "value": book["states"]["focus"]["ring_alpha"],
                "src": book["states"]["focus"]["src"],
            },
            "FOCUS_RING_OPACITY",
        ),
        (
            "disabled opacity",
            {
                "value": book["states"]["disabled"]["opacity"],
                "src": book["states"]["disabled"]["src"],
            },
            "DISABLED_OPACITY",
        ),
        (
            "settings card opacity",
            book["settings"]["card_opacity"],
            "SETTINGS_CARD_OPACITY",
        ),
        (
            "frame mix",
            {
                "value": book["themes"]["graphite-dark"]["frame"][
                    "mix_card_toward_foreground"
                ],
                "src": book["themes"]["graphite-dark"]["frame"]["src"],
            },
            "FRAME_MIX",
        ),
        (
            "input alpha",
            {
                "value": book["themes"]["graphite-dark"]["input"]["alpha"],
                "src": book["themes"]["graphite-dark"]["input"]["src"],
            },
            "INPUT_OPACITY",
        ),
    )
    pairs += [
        (label, entry["value"], entry["src"], name) for label, entry, name in opacities
    ]
    settings = book["settings"]
    pairs += [
        (
            "settings column max",
            settings["column_max"]["px"],
            settings["column_max"]["src"],
            "SETTINGS_COLUMN_MAX_WIDTH",
        ),
        (
            "settings side padding",
            settings["padding_side"]["px"],
            settings["padding_side"]["src"],
            "SETTINGS_PADDING_SIDE",
        ),
        (
            "settings top padding",
            settings["padding_top"]["px"],
            settings["padding_top"]["src"],
            "SETTINGS_PADDING_TOP",
        ),
        (
            "settings card padding x",
            settings["card_padding_x"]["px"],
            settings["card_padding_x"]["src"],
            "SETTINGS_CARD_PADDING_X",
        ),
        (
            "settings card padding y",
            settings["card_padding_y"]["px"],
            settings["card_padding_y"]["src"],
            "SETTINGS_CARD_PADDING_Y",
        ),
    ]
    for name, shadow in book["shadows"].items():
        constant = "SHADOW_" + name.upper()
        pairs += [
            (
                f"shadow {name} offset",
                shadow["offset_y"],
                shadow["src"],
                constant + ".offset_y",
            ),
            (f"shadow {name} blur", shadow["blur"], shadow["src"], constant + ".blur"),
            (
                f"shadow {name} opacity",
                shadow["black_alpha"],
                shadow["src"],
                constant + ".opacity",
            ),
        ]
    row, weights, motion = book["row"], book["weights"], book["motion"]
    pairs += [
        ("list row indent", row["indent_px"], row["src"], "ROW_INDENT"),
        ("list row chevron", row["chevron_px"], row["src"], "ROW_CHEVRON_SIZE"),
        ("list row icon", row["icon_px"], row["src"], "ROW_ICON_SIZE"),
        ("list row gap", row["gap_px"], row["src"], "ROW_GAP"),
        ("weight regular", weights["regular"], weights["src"], "WEIGHT_REGULAR"),
        ("weight medium", weights["medium"], weights["src"], "WEIGHT_MEDIUM"),
        (
            "weight dialog title",
            weights["dialog_title"],
            weights["src"],
            "WEIGHT_DIALOG_TITLE",
        ),
        (
            "spinner steps per turn",
            motion["spinner_steps_per_turn"],
            motion["src"],
            "SPINNER_STEPS_PER_TURN",
        ),
        (
            "spinner steps per second",
            motion["spinner_steps_per_second"],
            motion["src"],
            "SPINNER_STEPS_PER_SECOND",
        ),
    ]
    pairs += [
        (
            f"motion {key}",
            motion[f"{key}_ms"],
            motion["src"],
            f"MOTION_{key.upper()}_MS",
        )
        for key in ("fast", "medium", "slow")
    ]
    return pairs


def parse_shadows(source):
    """SHADOW_NAME.field values from the `ShadowToken { offset_y: .., blur: .., opacity: .. }` constants."""
    values = {}
    pattern = re.compile(
        r"pub const (SHADOW_\w+): ShadowToken = ShadowToken \{([^}]*)\}", re.DOTALL
    )
    for name, body in pattern.findall(source):
        for field, number in re.findall(r"(\w+):\s*([0-9.]+)", body):
            values[f"{name}.{field}"] = float(number)
    return values


def same(first, second):
    return abs(first - second) <= OPACITY_TOLERANCE


def compare_numbers(book, source):
    constants = parse_constants(source) | parse_shadows(source)
    lines, used = [], set()
    for label, wanted, src, name in rulebook_pairs(book):
        used.add(name)
        if name not in constants:
            lines.append(
                f"ONLY IN RULEBOOK: {label}: rulebook {wanted} ({src}), token module has no {name}"
            )
        elif not same(float(wanted), constants[name]):
            lines.append(
                f"DIFFERENT: {label}: rulebook {wanted} ({src}), tokens.rs {name} = {constants[name]:g}"
            )
    for name, value in constants.items():
        if name not in used:
            lines.append(f"ONLY IN TOKENS: {name} = {value:g}")
    return lines


def parse_theme_hex(text):
    """#rrggbb or #rrggbbaa to (rgb tuple, alpha 0 to 1)."""
    rgb = tuple(int(text[i : i + 2], 16) for i in (1, 3, 5))
    alpha = int(text[7:9], 16) / 255 if len(text) == 9 else 1.0
    return rgb, alpha


def compare_themes(book, branch):
    lines = []
    for name, path in THEME_PATHS.items():
        theme = json.loads(git_show(branch, path))["colors"]
        expected = book["themes"][name]
        for role in (
            "background",
            "card",
            "accent",
            "foreground",
            "muted_foreground",
            "err",
            "pressed",
            "ring",
            "primary",
            "primary_pressed",
        ):
            if theme[role].lower() != expected[role]["hex"]:
                lines.append(
                    f"DIFFERENT: {name} {role}: rulebook {expected[role]['hex']} ({expected[role]['src']}), theme file {theme[role]}"
                )
        for role in ("border", "input"):
            rgb, alpha = parse_theme_hex(theme[role])
            wanted = expected[role]
            want_rgb = tuple(int(wanted["over"][i : i + 2], 16) for i in (1, 3, 5))
            if rgb != want_rgb or abs(alpha - wanted["alpha"]) > 0.005:
                lines.append(
                    f"DIFFERENT: {name} {role}: rulebook {wanted['over']} at {wanted['alpha']:.0%} ({wanted['src']}), theme file {theme[role]} ({alpha:.1%})"
                )
        if "selection" not in theme:
            lines.append(
                f"ONLY IN RULEBOOK: {name} selection {expected['selection']['hex']} ({expected['selection']['src']}), theme file has no selection role"
            )
    return lines


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--branch", default="design-tokens")
    arguments = parser.parse_args()
    book = json.loads((SCRIPT_DIR / "rulebook-tokens.json").read_text(encoding="utf-8"))
    source = git_show(arguments.branch, TOKENS_PATH)
    lines = compare_numbers(book, source) + compare_themes(book, arguments.branch)
    print("\n".join(lines))
    print(
        f"{len(lines)} lines; compared with {arguments.branch}:{TOKENS_PATH} and its two Graphite theme files"
    )


if __name__ == "__main__":
    main()
