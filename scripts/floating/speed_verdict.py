#!/usr/bin/env python3
"""Judges a palette or Settings speed run (speed.py --scene palette or settings) against today's Clanky's numbers.

Usage: speed_verdict.py RESULT.json BASELINE.json

Prints one line, then exits 0 when every number is within its limit and 1 otherwise. The limits are those of the
terminal pane gate (the same ones, so a popover must not make the window slower than the old one) plus one of its own:
the first frame of the palette, or of the Settings window, comes within one refresh of the key press.

  cat 100 MB        at most 1.1 times today's
  key to screen     p50 and p99 at most today's plus 2 ms
  idle drawing      none in 10 s with the palette or Settings open
  first frame       at most one refresh (1000 / 60 ms) plus the same 2 ms of slack the key numbers get

Memory is printed and not judged: four panes hold more than the one pane the baseline was measured with.
"""

import json
import sys
from pathlib import Path

CAT_LIMIT = 1.1
KEY_SLACK_MS = 2
REFRESH_MS = 1000 / 60


def limits(baseline):
    """The checks, each a function of the measured median."""
    return {
        "cat_s": lambda n: n <= CAT_LIMIT * baseline["cat_s"]["median"],
        "key_p50_ms": lambda n: n <= baseline["key_p50_ms"]["median"] + KEY_SLACK_MS,
        "key_p99_ms": lambda n: n <= baseline["key_p99_ms"]["median"] + KEY_SLACK_MS,
        "idle_draws": lambda n: n == 0,
        "popover_ms": lambda n: n <= REFRESH_MS + KEY_SLACK_MS,
    }


def judge(result, baseline):
    """(verdict text, whether every number passed) for a result file's summary and a baseline's numbers."""
    summary = result["summary"]
    missing = [key for key in limits(baseline) if key not in summary]
    if missing:
        return (
            f"the run has no number for {', '.join(missing)}; was it run with --scene palette or settings?",
            False,
        )
    checks = limits(baseline)
    bad = [key for key, passes in checks.items() if not passes(summary[key]["median"])]
    text = ", ".join(
        f"{key} {summary[key]['median']:.2f}" for key in (*checks, "memory_mib")
    )
    if bad:
        return f"over the limit: {' '.join(bad)}; {text}", False
    return f"five numbers within the limits: {text}", True


def main():
    if len(sys.argv) != 3:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    result = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    baseline = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))["numbers"]
    verdict, passed = judge(result, baseline)
    print(verdict)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
