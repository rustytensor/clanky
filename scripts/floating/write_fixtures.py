#!/usr/bin/env python3
"""Writes the committed fixtures of G3's floating screens: for each screen and scale, the window with the popover open
(PNG) and the record's lines for the popover's screen (JSONL), from the simulated floating window; and the speed
results the verdict script is proven on.

Usage: python3 write_fixtures.py --out-dir DIR [--theme graphite-dark]
"""

import argparse
import json
import sys
from pathlib import Path

from PIL import Image

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
from simulated_floating import SimFloatingWindow  # noqa: E402

SCALES = (1, 1.5)
BASELINE = SCRIPT_DIR.parent / "terminal-pane/baseline.json"
OPENERS = {
    "palette": lambda win: win.key("ctrl+shift+p"),
    "agent-picker": lambda win: win.key("ctrl+shift+g"),
    "completion-list": lambda win: (win.type("ls scripts/gate-m"), win.key("Tab")),
    "find-bar": lambda win: (win.key("ctrl+shift+f"), win.type("g1")),
    "context-menu": lambda win: win.x("click", "3"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--theme", default="graphite-dark")
    arguments = parser.parse_args()
    arguments.out_dir.mkdir(parents=True, exist_ok=True)
    for scale in SCALES:
        for screen, open_it in OPENERS.items():
            window = SimFloatingWindow(scale, arguments.theme)
            window.move(*window.menu_point())
            open_it(window)
            name = f"{screen}-good-{scale:g}"
            Image.fromarray(window.shot(name).astype("uint8")).save(
                arguments.out_dir / f"{name}.png"
            )
            lines = [window.screen(screen)]
            text = "\n".join(json.dumps(line, sort_keys=True) for line in lines) + "\n"
            (arguments.out_dir / f"{name}.jsonl").write_text(text, encoding="utf-8")
    write_speed_fixtures(arguments.out_dir)


SPEED_FAULTS = {  # the number a slow run gets wrong, and the value it shows
    "idle_draws": 3,
    "cat_s": 80,
    "key_p50_ms": 40,
    "popover_ms": 40,
}


def write_speed_fixtures(out_dir):
    """Speed results for speed_verdict.py: a good run built from today's baseline, and one slow number each."""
    baseline = json.loads(BASELINE.read_text(encoding="utf-8"))["numbers"]
    good = {key: {"median": entry["median"]} for key, entry in baseline.items()}
    good["idle_draws"] = {"median": 0}
    good["popover_ms"] = {"median": 12.0}
    runs = {"speed-good": good}
    for key, value in SPEED_FAULTS.items():
        runs[f"speed-slow-{key.split('_')[0]}"] = {**good, key: {"median": value}}
    for name, summary in runs.items():
        (out_dir / f"{name}.json").write_text(
            json.dumps({"summary": summary}, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
