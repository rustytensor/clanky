#!/bin/bash
# M12 check 15: real windows and real input, with a harness-controlled clock.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CARGO_TARGET_DIR:?Set CARGO_TARGET_DIR to the team build directory}"
export CARGO_TARGET_DIR="$(realpath -m "$CARGO_TARGET_DIR")"
export CARGO_HOME="${CARGO_HOME:-/srv/devops/cache/clanky-cargo}"
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json LIBGL_ALWAYS_SOFTWARE=1 LP_NUM_THREADS=8
mkdir -p "$CARGO_TARGET_DIR"
exec 9>"${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}"
flock 9
if [[ ${CLANKY_GATE_BINARIES_READY:-0} != 1 ]]; then
  # With CLANKY_BIN only the owner test client is needed; today's clanky is not built.
  if [[ -n ${CLANKY_BIN:-} ]]; then cargo build --offline -p clanky --bin clanky-agent-gate -j8
  else cargo build --offline -p clanky --bin clanky --bin clanky-agent-gate -j8; fi
fi
work=$(mktemp -d "$CARGO_TARGET_DIR/robot-window.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
xvfb-run -a -n 100 -f "$work/xauthority" -e "$work/xvfb.log" -s '-screen 0 1400x900x24 -nolisten tcp' \
  python3 scripts/check-robot-window.py "$work" "${CLANKY_GATE_BIN_DIR:-$CARGO_TARGET_DIR/debug}"
