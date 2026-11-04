#!/usr/bin/env bash
set -euo pipefail
trap 'status=$?; if (( status != 0 )); then echo "FAIL: M7 editor core" >&2; fi' EXIT
: "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}"
: "${CARGO_HOME:?set CARGO_HOME}"
: "${CLANKY_BUILD_LOCK:=/srv/devops/build/clanky/.build.lock}"
case "${CLANKY_GATE_FAULT:-}" in
    ''|accepted) ;;
    *) echo "FAIL: unknown CLANKY_GATE_FAULT" >&2; exit 1 ;;
esac
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
if [[ ${CLANKY_GATE_BINARIES_READY:-} != 1 ]]; then
    flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky-edit --bin editor-core-gate -j 8
fi
"$CARGO_TARGET_DIR/debug/editor-core-gate"
