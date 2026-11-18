#!/usr/bin/env python3
"""Photograph the main window in each of the six themes (Graphite, Neutral, Zinc; dark and light) on a private display.

    scripts/themes/shoot_themes.py [--app PATH] [--out DIR] [--scale S]

One real clanky-gpui window per theme, started the way the window chrome gates start it (scripts/window-chrome/
chrome_harness.py), with the theme named in its settings file. Writes DIR/<theme>.png. Run it under the gate lock
(`flock /srv/devops/build/clanky/.gate.lock`), because it starts a window and a display.
"""

import argparse
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "window-chrome"))
from chrome_harness import RealWindow  # noqa: E402

THEMES = (
    ("graphite-dark", "dark"),
    ("neutral-dark", "dark"),
    ("zinc-dark", "dark"),
    ("graphite-light", "light"),
    ("neutral-light", "light"),
    ("zinc-light", "light"),
)
BASE_CONFIG = {
    "restore_layout": False,
    "new_window_panes": 1,
    "startup_animation": False,
    "history": False,
    "animations": False,
    "cursor_blink": "off",
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--app", type=Path, default=HERE.parents[1] / "target/release/clanky-gpui"
    )
    parser.add_argument(
        "--out", type=Path, default=Path("/srv/devops/build/clanky-themes")
    )
    parser.add_argument("--scale", type=float, default=1.0)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    work = Path("/srv/devops/build/clanky-themes-work")
    for theme, mode in THEMES:
        config = {**BASE_CONFIG, "theme": mode, f"theme_{mode}": theme}
        shutil.rmtree(work / theme, ignore_errors=True)
        window = RealWindow(args.scale, work / theme, args.app, config, keep=True)
        try:
            window.shot(theme)
            shutil.copy2(window.work / f"{theme}.png", args.out / f"{theme}.png")
            print(f"wrote {args.out / theme}.png")
        finally:
            window.close()
    shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
