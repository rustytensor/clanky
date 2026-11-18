#!/usr/bin/env python3
"""The six speed numbers of one speed.sh run against the baseline's, with the limits the terminal pane gate uses.

    scripts/parity/speed_verdict.py RUN.json BASELINE.json TIME_LIMIT KEY_MS_LIMIT START_LIMIT

Exit 1 when a number is over its limit. The limits are the terminal pane gate's (SPEED_LIMIT_* in
scripts/gate-terminal-pane.sh), passed in so that there is one place they are written.
"""

import json
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    now = json.loads(Path(argv[0]).read_text())["summary"]
    then = json.loads(Path(argv[1]).read_text())["numbers"]
    time_limit, key_limit, start_limit = (float(value) for value in argv[2:5])
    limits = {
        "cat_s": lambda n, t: n <= time_limit * t,
        "key_p50_ms": lambda n, t: n <= t + key_limit,
        "key_p99_ms": lambda n, t: n <= t + key_limit,
        "idle_draws": lambda n, t: n == 0,
        "start_ms": lambda n, t: n <= start_limit * t,
        "memory_mib": lambda n, t: n <= t,
    }
    rows = {key: (now[key]["median"], then[key]["median"]) for key in limits}
    text = ", ".join(
        f"{key} {n:.2f} (baseline {t:.2f})" for key, (n, t) in rows.items()
    )
    bad = [key for key, (n, t) in rows.items() if not limits[key](n, t)]
    print(
        (
            "over the limit: " + " ".join(bad) + "; "
            if bad
            else "six numbers within the limits: "
        )
        + text
    )
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
