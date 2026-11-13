#!/usr/bin/env bash
# Q3 gate: prints "PASS: Q3 syntax" or fails. --faults plants six faults and passes only if all are caught.
# Run it under the window-gate lock: flock /srv/devops/build/clanky/.gate.lock scripts/gate-q3-syntax.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
if [[ ${1:-} == --faults ]]; then
    caught=0
    for fault in table theme edit italic idle language; do
        if out=$(python3 scripts/gate-q3-syntax.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault ($(grep -m1 '^FAIL' <<<"$out"))"; caught=$((caught + 1))
        fi
    done
    [[ $caught == 6 ]] && { echo 'PASS: Q3 gate catches all 6 planted faults'; exit 0; }
    exit 1
fi
exec python3 scripts/gate-q3-syntax.py
