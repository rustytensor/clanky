#!/usr/bin/env bash
set -euo pipefail
trap 'status=$?; if (( status != 0 )); then echo "FAIL: terminal cwd" >&2; fi' EXIT
: "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}"
: "${CARGO_HOME:?set CARGO_HOME}"
: "${CLANKY_BUILD_LOCK:=/srv/devops/build/clanky/.build.lock}"
case "${CLANKY_GATE_FAULT:-}" in
    ''|cwd) ;;
    *) echo "FAIL: unknown CLANKY_GATE_FAULT" >&2; exit 1 ;;
esac
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
umask 077
scratch=$(mktemp -d "$CARGO_TARGET_DIR/clanky-cwd.XXXXXXXX")
trap 'status=$?; rm -rf -- "$scratch"; if (( status != 0 )); then echo "FAIL: terminal cwd" >&2; fi' EXIT
# Snapshot under the same lock as the build, so concurrent builders cannot
# replace the binary between the parent and its self-child server launch.
(
    flock 9
    cargo build --offline --locked -p clanky --bin clanky-cwd-gate -j 8
    cp -- "$CARGO_TARGET_DIR/debug/clanky-cwd-gate" "$scratch/clanky-cwd-gate"
) 9>"$CLANKY_BUILD_LOCK"
"$scratch/clanky-cwd-gate" "$scratch"
