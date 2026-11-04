#!/usr/bin/env bash
set -euo pipefail
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: session polish" >&2; fi' EXIT
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=/srv/devops/build/clanky-pkg/target
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CLANKY_BUILD_LOCK=/srv/devops/build/clanky/.build.lock
case ${CLANKY_GATE_FAULT:-} in ''|polish) ;; *) echo 'FAIL: unknown fault' >&2; exit 1;; esac
flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky --bin clanky --bin polish-session-gate -j 8
python3 scripts/gate-polish-session.py "$CARGO_TARGET_DIR/debug/polish-session-gate" "$CARGO_TARGET_DIR/debug/clanky"
if [[ ${CLANKY_RC_NO_NESTED:-0} != 1 ]]; then
    env -u CLANKY_GATE_FAULT scripts/gate-m9.sh
fi
echo 'PASS: session polish'
