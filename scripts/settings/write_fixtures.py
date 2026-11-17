#!/usr/bin/env python3
"""Writes the committed fixtures of the Settings window: for each page and scale, the window with that page open
(PNG) and the record's lines for it (JSONL: the `settings` screen line and the landmarks), from the simulated Settings
window; and the speed results the verdict script is proven on.

Usage: python3 write_fixtures.py --out-dir DIR [--theme graphite-dark]
"""

import argparse
import json
import sys
from pathlib import Path

from PIL import Image

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
from simulated_settings import SimSettingsWindow  # noqa: E402

SCALES = (1, 1.5)
PAGES = ("General", "Appearance", "Terminal", "Editor", "Agents", "History", "Keys")
BASELINE = SCRIPT_DIR.parent / "terminal-pane/baseline.json"


def open_page(window, page):
    """Opens Settings with its shortcut and picks the page through the switcher, as a person would."""
    window.key("ctrl+comma")
    segment = next(
        control
        for control in window.screen("settings")["controls"]
        if control["group"] == "settings-pages" and control["label"] == page
    )
    x, y, width, height = segment["box"]
    window.move(x + width // 2, y + height // 2)
    window.x("mousedown", "1")
    window.x("mouseup", "1")
    window.away()
    if window.screen("settings")["page"] != page:
        raise RuntimeError(f"Settings did not show the {page} page")


def record_lines(window):
    """The `settings` screen line, then the landmark lines the window last wrote, in name and index order."""
    lines = [window.screen("settings")]
    for name, boxes in sorted(window.landmarks().items()):
        for index, box in enumerate(boxes):
            lines.append(
                {"record": "landmark", "name": name, "index": index, "box": box}
            )
    return lines


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--theme", default="graphite-dark")
    arguments = parser.parse_args()
    arguments.out_dir.mkdir(parents=True, exist_ok=True)
    for scale in SCALES:
        for page in PAGES:
            window = SimSettingsWindow(scale, arguments.theme)
            open_page(window, page)
            name = f"settings-{page.lower()}-good-{scale:g}"
            Image.fromarray(window.shot(name).astype("uint8")).save(
                arguments.out_dir / f"{name}.png"
            )
            text = "\n".join(
                json.dumps(line, sort_keys=True) for line in record_lines(window)
            )
            (arguments.out_dir / f"{name}.jsonl").write_text(
                text + "\n", encoding="utf-8"
            )
    write_speed_fixtures(arguments.out_dir)


SPEED_FAULTS = {  # the number a slow run gets wrong, and the value it shows
    "idle_draws": 3,
    "cat_s": 80,
    "key_p50_ms": 40,
    "popover_ms": 40,
}


def write_speed_fixtures(out_dir):
    """Speed results for speed_verdict.py: a good run built from today's baseline, and one slow number each.
    `popover_ms` is the time from the Ctrl+comma press to the Settings window's first frame."""
    baseline = json.loads(BASELINE.read_text(encoding="utf-8"))["numbers"]
    good = {key: {"median": entry["median"]} for key, entry in baseline.items()}
    good["idle_draws"] = {"median": 0}
    good["popover_ms"] = {"median": 12.0}
    runs = {"speed-settings-good": good}
    for key, value in SPEED_FAULTS.items():
        runs[f"speed-settings-slow-{key.split('_')[0]}"] = {
            **good,
            key: {"median": value},
        }
    for name, summary in runs.items():
        (out_dir / f"{name}.json").write_text(
            json.dumps({"summary": summary}, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
