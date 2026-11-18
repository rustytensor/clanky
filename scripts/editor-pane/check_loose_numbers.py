#!/usr/bin/env python3
"""The design gate's check 1 (no loose numbers) on one source file, without a window.

Usage: check_loose_numbers.py FILE.rs
Prints "loose-numbers FILE: 0 loose numbers" and exits 0, or "loose-numbers FILE: N loose numbers (LINE KIND: TEXT; ...)"
and exits 1. The rules (which calls, which literals, `// gate: allow` comments) are gate-design-tokens.py's own `scan`.
"""

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DESIGN_GATE = HERE.parent / "window-chrome/gate-design-tokens.py"


def main():
    if len(sys.argv) != 2:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    spec = importlib.util.spec_from_file_location("design_gate", DESIGN_GATE)
    design_gate = importlib.util.module_from_spec(spec)
    argv, sys.argv = sys.argv, [sys.argv[0]]
    spec.loader.exec_module(design_gate)
    sys.argv = argv
    hits, _ = design_gate.scan({str(path): path.read_text(encoding="utf-8")})
    if hits:
        listed = "; ".join(f"{line} {kind}: {text}" for _, line, kind, text in hits[:5])
        print(f"loose-numbers {path}: {len(hits)} loose numbers ({listed})")
        return 1
    print(f"loose-numbers {path}: 0 loose numbers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
