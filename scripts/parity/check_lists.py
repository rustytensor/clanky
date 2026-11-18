#!/usr/bin/env python3
"""Every gate script is on one list: gates.txt (run by gate-parity.sh), retired.txt, or other.txt.

    scripts/parity/check_lists.py [--root DIR]

A gate script is a file in DIR/scripts whose name starts with gate- or check-; a gate pairs a .sh and a .py of one stem,
so the stem is what is listed. Each list line is `stem<TAB>words` (gates.txt has `stem<TAB>command<TAB>covers`) and
the words are never empty. Exit 1 with the first problem: a script on no list, on two lists, a listed stem with no
script, a malformed line.
"""

import argparse
import sys
from pathlib import Path

LISTS = {
    "gates.txt": 3,
    "retired.txt": 2,
    "other.txt": 2,
}  # file name: the number of tab-separated columns


class ListError(Exception):
    """The first way the lists disagree with the scripts."""


def gate_stems(scripts: Path) -> set[str]:
    return {
        path.name.split(".")[0]
        for path in scripts.iterdir()
        if path.is_file() and path.name.startswith(("gate-", "check-"))
    }


def read_list(path: Path, columns: int) -> dict[str, list[str]]:
    rows: dict[str, list[str]] = {}
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != columns or not all(field.strip() for field in fields):
            raise ListError(
                f"{path}:{number} needs {columns} non-empty tab-separated columns: {line[:80]!r}"
            )
        if fields[0] in rows:
            raise ListError(f"{path}:{number} lists {fields[0]} twice")
        rows[fields[0]] = fields
    return rows


def check(root: Path) -> str:
    scripts = root / "scripts"
    stems = gate_stems(scripts)
    listed: dict[str, str] = {}
    for name, columns in LISTS.items():
        for stem in read_list(scripts / "parity" / name, columns):
            if stem in listed:
                raise ListError(f"{stem} is on both {listed[stem]} and {name}")
            if stem not in stems:
                raise ListError(
                    f"{name} lists {stem}, but scripts/ has no gate-/check- script of that name"
                )
            listed[stem] = name
    missing = sorted(stems - set(listed))
    if missing:
        raise ListError(
            f"scripts/{missing[0]}.* is a gate script on no list (gates.txt, retired.txt, other.txt)"
        )
    counts = {
        name: sum(1 for value in listed.values() if value == name) for name in LISTS
    }
    return (
        f"{len(stems)} gate scripts: {counts['gates.txt']} run, {counts['retired.txt']} retired, "
        f"{counts['other.txt']} other"
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent.parent
    )
    arguments = parser.parse_args()
    try:
        print(f"PASS: {check(arguments.root)}")
    except (ListError, OSError) as problem:
        print(f"FAIL: {problem}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
