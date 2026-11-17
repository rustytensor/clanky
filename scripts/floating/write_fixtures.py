#!/usr/bin/env python3
"""Writes the committed fixtures of G3's floating screens: for each screen and scale, the window with the popover open
(PNG) and the record's lines for the popover's screen (JSONL), from the simulated floating window.

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
OPENERS = {
    "palette": lambda win: win.key("ctrl+shift+p"),
    "agent-picker": lambda win: win.key("ctrl+shift+g"),
    "completion-list": lambda win: (win.type("ls scripts/gate-m"), win.key("Tab")),
    "find-bar": lambda win: (win.key("ctrl+f"), win.type("g1")),
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


if __name__ == "__main__":
    main()
