#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$ROOT/scripts/gate-render-env.sh"
: "${CARGO_TARGET_DIR:?set an absolute CARGO_TARGET_DIR}"
: "${CARGO_HOME:?set an absolute CARGO_HOME}"
case "$CARGO_TARGET_DIR" in /*) ;; *) echo 'FAIL: CARGO_TARGET_DIR must be absolute' >&2; exit 1;; esac
case "$CARGO_HOME" in /*) ;; *) echo 'FAIL: CARGO_HOME must be absolute' >&2; exit 1;; esac
export CLANKY_BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
case "$CLANKY_BUILD_LOCK" in /*) ;; *) echo 'FAIL: CLANKY_BUILD_LOCK must be absolute' >&2; exit 1;; esac
case "${CLANKY_GATE_FAULT:-}" in ''|editor-save) ;; *) echo 'FAIL: unknown panel fault' >&2; exit 1;; esac
cd "$ROOT"
mkdir -p -- "$CARGO_TARGET_DIR" "$(dirname -- "$CLANKY_BUILD_LOCK")"
# Release the lock before the M4 Python gate captures its prebuilt executables.
(
    flock -x 9
    cargo build --offline --locked --workspace --bins -j 8
) 9>"$CLANKY_BUILD_LOCK"
"$CARGO_TARGET_DIR/debug/clanky-panel-gate"
# A planted fault exits at the fixed save-byte comparison above, never here.
if [ -n "${CLANKY_GATE_FAULT:-}" ]; then
    echo 'FAIL: planted save fault was not caught' >&2
    exit 1
fi
"$ROOT/scripts/gate-m4.sh"
echo 'PASS: M7 editor panel (including M4/M3/M2/M1)'
