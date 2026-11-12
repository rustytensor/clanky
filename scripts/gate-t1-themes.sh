#!/usr/bin/env bash
# T1 gate: prints "PASS: T1 themes" or fails. --faults plants three faults and passes only if all are caught.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" \
    cargo build -q --offline --locked -j 8 -p clanky --bin clanky-theme-gate
palettes=$("$CARGO_TARGET_DIR/debug/clanky-theme-gate" --palettes)
if [[ ${1:-} == --faults ]]; then
    caught=0
    for fault in gradient ansi background; do
        if CLANKY_T1_FAULT=$fault python3 scripts/gate-t1-themes.py <<<"$palettes" > /dev/null; then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault"; caught=$((caught + 1))
        fi
    done
    [[ $caught == 3 ]] && echo 'PASS: T1 gate catches all 3 planted faults' || exit 1
    exit 0
fi
python3 scripts/gate-t1-themes.py <<<"$palettes"
