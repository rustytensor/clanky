#!/usr/bin/env bash
set -euo pipefail
ulimit -c 0
# Never capture runtime output in files: the binary captures and scans in memory.
fail() { printf '%s\n' 'FAIL: masking gate setup'; exit 1; }
case "${CARGO_TARGET_DIR:-}" in /*) ;; *) fail ;; esac
case "${CARGO_HOME:-}" in /*) ;; *) fail ;; esac
case "${CLANKY_GATE_FAULT:-}" in ''|split) ;; *) fail ;; esac
lock="${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}"
case "$lock" in /*) ;; *) fail ;; esac
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." || fail
# Source contains no credential values; cargo output is safe, runtime output is not.
if ! flock "$lock" cargo build --offline --locked -p clanky-mask --bin masking-gate -j8; then
    printf '%s\n' 'FAIL: masking gate build'
    exit 1
fi
exec "$CARGO_TARGET_DIR/debug/masking-gate"
