#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
: "${CARGO_TARGET_DIR:?set an absolute CARGO_TARGET_DIR}"
: "${CARGO_HOME:?set an absolute CARGO_HOME}"
: "${VK_DRIVER_FILES:?lead must supply the software Vulkan driver}"
: "${LIBGL_ALWAYS_SOFTWARE:?lead must force software rendering}"
case "$CARGO_TARGET_DIR" in /*) ;; *) echo 'FAIL: CARGO_TARGET_DIR must be absolute' >&2; exit 1;; esac
case "$CARGO_HOME" in /*) ;; *) echo 'FAIL: CARGO_HOME must be absolute' >&2; exit 1;; esac
case "$LIBGL_ALWAYS_SOFTWARE" in 1|true) ;; *) echo 'FAIL: software rendering required' >&2; exit 1;; esac
export CLANKY_BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
case "$CLANKY_BUILD_LOCK" in /*) ;; *) echo 'FAIL: CLANKY_BUILD_LOCK must be absolute' >&2; exit 1;; esac
case "${CLANKY_GATE_FAULT:-}" in ''|compose-send) ;; *) echo 'FAIL: unknown compose fault' >&2; exit 1;; esac
cd "$ROOT"
mkdir -p -- "$CARGO_TARGET_DIR" "$(dirname -- "$CLANKY_BUILD_LOCK")"
(
    flock -x 9
    cargo build --offline --locked --workspace --bins -j 8
) 9>"$CLANKY_BUILD_LOCK"
"$CARGO_TARGET_DIR/debug/clanky-compose-gate"
# Fault must exit at the observed PTY comparison, before any regressions.
if [ -n "${CLANKY_GATE_FAULT:-}" ]; then
    echo 'FAIL: planted compose fault was not caught' >&2
    exit 1
fi
"$ROOT/scripts/gate-m7-panel.sh"
"$ROOT/scripts/gate-m7-core.sh"
"$ROOT/scripts/gate-m6-masking.sh"
"$ROOT/scripts/gate-terminal-cwd.sh"
echo 'PASS: M8 compose (including panel/M1-M4, editor core, masking and cwd once)'
