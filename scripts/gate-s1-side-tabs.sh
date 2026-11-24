#!/usr/bin/env bash
# Side tabs gate: clanky-gpui's side column (tab_style vertical), its rows, groups, states, limits and idle drawing.
# Prints "PASS <n> <name>" or "FAIL <n> <name>: <why>" per gate, then PASS (exit 0) or FAIL (exit 1). The gates and
# their expected values are in scripts/side-tabs/gate_side_tabs.py.
#
# Usage: scripts/gate-s1-side-tabs.sh [--only N,N...] [--keep]
#   Builds clanky-gpui (release, with the control-faults feature, which behaves as a release build until a fault is
#   planted) and the clanky server, unless CLANKY_GPUI_BIN names a binary already built that way.
#   CLANKY_GATE_FAULT=side-width plants the window's side-width fault (the column 240 px wide) and runs gate 3 only;
#   that run must print FAIL. The full run does this itself as gate 18.
set -euo pipefail
cd "$(dirname "$0")/.."

binary=${CLANKY_GPUI_BIN:-}
if [[ -z $binary ]]; then
  if ! log=$(cargo build -q --release --locked -j 8 -p clanky-gpui -p clanky --bin clanky-gpui --bin clanky \
    --features clanky-gpui/control-faults 2>&1); then
    echo "FAIL 0 build: $(tail -n 3 <<<"$log" | tr '\n' ' ')"
    echo "FAIL"
    exit 1
  fi
  binary=${CARGO_TARGET_DIR:-target}/release/clanky-gpui
fi
args=(--app "$(realpath "$binary")")
if [[ -n ${CLANKY_GATE_FAULT:-} ]]; then
  args+=(--fault "$CLANKY_GATE_FAULT")
fi
exec env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
  python3 scripts/side-tabs/gate_side_tabs.py "${args[@]}" "$@"
