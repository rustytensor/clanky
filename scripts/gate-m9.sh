#!/usr/bin/env bash
set -euo pipefail
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: M9 TUI" >&2; fi' EXIT
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=/srv/devops/build/clanky-pkg/target
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CLANKY_BUILD_LOCK=/srv/devops/build/clanky/.build.lock
case ${CLANKY_GATE_FAULT:-} in ''|tui) ;; *) echo 'FAIL: unknown fault' >&2; exit 1;; esac
flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky --bin clanky -j 8
python3 scripts/gate-m9.py "$CARGO_TARGET_DIR/debug/clanky"
if [[ ${CLANKY_RC_NO_NESTED:-0} != 1 ]]; then
    env -u CLANKY_GATE_FAULT scripts/gate-m7-core.sh
fi
echo 'PASS: M9 TUI'
