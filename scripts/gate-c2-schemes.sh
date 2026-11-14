#!/usr/bin/env bash
# C2 gate (terminal colour schemes): prints "PASS: C2 terminal colour schemes" or fails. --faults plants seven faults
# and passes only if all are caught. Run under the gate lock:
#   flock /srv/devops/build/clanky/.gate.lock scripts/gate-c2-schemes.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
if [[ ${1:-} == --faults ]]; then
    caught=0
    for fault in dots escape follow source idle import bad; do
        if out=$(python3 scripts/gate-c2-schemes.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault ($(grep -m1 '^FAIL' <<<"$out"))"; caught=$((caught + 1))
        fi
    done
    [[ $caught == 7 ]] && { echo 'PASS: C2 gate catches all 7 planted faults'; exit 0; }
    exit 1
fi
exec python3 scripts/gate-c2-schemes.py
