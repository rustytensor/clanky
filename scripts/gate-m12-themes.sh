#!/usr/bin/env bash
# Theme contrast and real storage round trips; independent of the GUI/full gate.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-all}
if (( $# > 1 )); then echo 'usage: gate-m12-themes.sh [--contrast-only|--storage-only]' >&2; exit 2; fi
case "$mode" in all|--contrast-only|--storage-only) ;; *) echo 'usage: gate-m12-themes.sh [--contrast-only|--storage-only]' >&2; exit 2;; esac
case ${CLANKY_THEME_GATE_FAULT:-} in ''|contrast|storage) ;; *) echo 'FAIL: unknown theme fault' >&2; exit 2;; esac
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}
mkdir -p "$CARGO_TARGET_DIR"
export CARGO_TARGET_DIR=$(realpath "$CARGO_TARGET_DIR")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" \
    cargo build --offline --locked -j 8 -p clanky --bin clanky-theme-gate
status=0
if [[ $mode != --storage-only ]]; then
    "$CARGO_TARGET_DIR/debug/clanky-theme-gate" --palettes | python3 scripts/gate-m12-theme-contrast.py || status=1
fi
# Do not hide storage results behind an existing palette contrast failure.
if [[ $mode != --contrast-only ]]; then
    "$CARGO_TARGET_DIR/debug/clanky-theme-gate" --storage || status=1
fi
if (( status == 0 )); then echo 'PASS: M12 theme checks'; else echo 'FAIL: M12 theme checks' >&2; fi
exit "$status"
