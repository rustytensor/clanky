#!/usr/bin/env bash
# Menu gate: prints "PASS: menu" or fails. --faults plants five faults and passes only if all are caught.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
if [[ ${1:-} == --faults ]]; then
    caught=0
    for fault in copy shift link reset postai; do
        if out=$(python3 scripts/gate-menu.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault ($(grep -m1 '^FAIL' <<<"$out"))"; caught=$((caught + 1))
        fi
    done
    [[ $caught == 5 ]] && { echo 'PASS: menu gate catches all 5 planted faults'; exit 0; }
    exit 1
fi
exec python3 scripts/gate-menu.py
