# M12 robot: real-window check 15

Run `scripts/check-robot-window.sh` with the caller's `CARGO_TARGET_DIR` set.
It builds `clanky` and `clanky-agent-gate` with eight workers under the shared lock,
then uses a private Xvfb display, home, config, state directory and server.
Requires Xvfb/xvfb-run, xdotool and Python Pillow with XCB capture support.

For an already-built matching binary, set `CLANKY_GATE_BINARIES_READY=1`.
`CLANKY_GATE_BIN_DIR` optionally selects captured binaries instead of `$CARGO_TARGET_DIR/debug`.
The clock hooks must be in that build. Call outside any outer build-lock scope; this script takes the lock itself.
No milestone/regression suite runs here.

- Normal windows use the real frame scheduler, first-run marker and input handler.
- The first window has a 7000 ms clock; the next has 1200 ms. Initial housekeeping settles
  with the animation clock frozen at zero. Advancing beyond completion produces zero frames,
  checked through another 2000 logical ms and two real quiet seconds.
- A real X11 key event at logical 300 ms skips before the next frame and delivers exactly `x`
  to a real PTY reader. The clock stays frozen at 300 during input dispatch.
- A real `+hello` window supplies frozen pixel captures, including the final pose at 7000 ms.
  Normal startup clears its overlay at that deadline; replay intentionally retains the pose.
  All pixels come from X11 screen capture, not offscreen rendering.

The replay window is 448 × 448, leaving the 400 × 400 robot canvas inside 24 px padding.
At 7000 ms, antenna `(215,73)` must be RGB `(210,164,118)`, tolerance 3 per channel.
Base `(194,324)` at 5850 ms must differ from 5400 and 6300 ms; torso `(202,229)` at
3200 ms must differ from 2400 and 4000 ms. Coordinates come from the reference geometry.

`CLANKY_GATE_FAULT=robot scripts/check-robot-window.sh` forces continued redraws and must
fail the zero-frame check. It does not change an expected value.

The opt-in `CLANKY_ROBOT_GATE=1` / `CLANKY_ROBOT_CLOCK` pair enables a bounded, monotonic
clock-file input. Without that pair, windows use the ordinary clock and do no clock-file I/O.
The gate feeds the same FrameClock and startup controller as the normal window.

Results: `$CARGO_TARGET_DIR/robot-window-result.json`; real-window images:
`$CARGO_TARGET_DIR/look/robot/frame-<ms>.png`. All fixture processes and scratch files are
removed. The real first-run marker and desktop session are untouched.
