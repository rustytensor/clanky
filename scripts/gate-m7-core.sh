#!/usr/bin/env bash
set -euo pipefail
trap 'status=$?; if (( status != 0 )); then echo "FAIL: M7 editor core" >&2; fi' EXIT
: "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}"
: "${CARGO_HOME:?set CARGO_HOME}"
: "${CLANKY_BUILD_LOCK:?set CLANKY_BUILD_LOCK}"
case "${CLANKY_GATE_FAULT:-}" in
    ''|accepted) ;;
    *) echo "FAIL: unknown CLANKY_GATE_FAULT" >&2; exit 1 ;;
esac
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky-edit --bin editor-core-gate -j 8
"$CARGO_TARGET_DIR/debug/editor-core-gate"
