#!/bin/bash
# Focused startup integration check; never runs the milestone/regression suites.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CARGO_TARGET_DIR:?Set CARGO_TARGET_DIR to the team build directory}"
export CARGO_HOME="${CARGO_HOME:-/srv/devops/cache/clanky-cargo}"
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json LIBGL_ALWAYS_SOFTWARE=1 LP_NUM_THREADS=8
mkdir -p "$CARGO_TARGET_DIR"
exec 9>"${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}"
flock 9
cargo test --offline -p clanky --lib robot::tests::startup_schedule -j8 -- --exact --test-threads=1 --nocapture
cargo build --offline -p clanky --bin clanky --bin clanky-agent-gate -j8
work=$(mktemp -d "$CARGO_TARGET_DIR/robot-startup.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
xvfb-run -a -f "$work/xauthority" -e "$work/xvfb.log" -s '-screen 0 1400x900x24 -nolisten tcp' \
  python3 scripts/check-robot-startup.py "$work" "$CARGO_TARGET_DIR/debug"
