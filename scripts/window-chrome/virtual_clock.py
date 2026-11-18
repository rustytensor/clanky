"""A clock for the simulated gates: sleeping moves the clock forward instead of waiting.

A simulated window answers at once, so every pause the gates take to let a real window settle is wasted time there
(44 of the 54 seconds of one simulated controls run). With this installed `time.sleep` adds its length to the clock and
returns, and `time.monotonic` reads the real clock plus what was skipped, so the code that measures time (waiting
loops with a deadline, the frames the simulated window draws per second) sees the same durations as before.
Real-window runs never install it.
"""

import time

_real_monotonic = time.monotonic
_skipped_seconds = 0.0


def _sleep(seconds):
    global _skipped_seconds
    _skipped_seconds += max(float(seconds), 0.0)


def _monotonic():
    return _real_monotonic() + _skipped_seconds


def install():
    """Replaces time.sleep and time.monotonic for the rest of the process."""
    time.sleep = _sleep
    time.monotonic = _monotonic
