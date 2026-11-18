#!/usr/bin/env python3
"""No crate in the workspace depends on the old window's libraries, and the old window's folder is gone.

    scripts/parity/check_dependencies.py [--root DIR]

Reads `cargo metadata` of DIR (offline, with the caller's CARGO_HOME). A workspace crate fails when it depends, as a
normal or build dependency, on winit, cosmic-text or the old renderer crate (clanky-render), or is itself the old
renderer; the tree fails while crates/clanky/src/calm exists. Exit 1 listing every offender.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

FORBIDDEN = ("winit", "cosmic-text", "clanky-render")
OLD_WINDOW_FOLDER = Path("crates/clanky/src/calm")


def offenders(root: Path) -> list[str]:
    done = subprocess.run(
        [
            "cargo",
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
            str(root / "Cargo.toml"),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode:
        lines = done.stderr.strip().splitlines()
        return [
            f"cargo metadata failed in {root}: {lines[-1] if lines else 'no message'}"
        ]
    found = []
    for package in json.loads(done.stdout)["packages"]:
        if package["name"] in FORBIDDEN:
            found.append(f"{package['name']} is still a workspace crate")
        for dependency in package["dependencies"]:
            if dependency["name"] in FORBIDDEN and dependency["kind"] != "dev":
                found.append(f"{package['name']} depends on {dependency['name']}")
    if (root / OLD_WINDOW_FOLDER).exists():
        found.append(f"{OLD_WINDOW_FOLDER} still exists")
    return found


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent.parent
    )
    found = offenders(parser.parse_args().root)
    if found:
        print("FAIL: " + "; ".join(found))
        return 1
    print(
        f"PASS: no workspace crate depends on {', '.join(FORBIDDEN)}, and the old window's folder is gone"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
