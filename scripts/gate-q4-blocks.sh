#!/usr/bin/env bash
# Q4 gate (breathing blocks): prints "PASS: Q4 breathing blocks" or fails. --faults plants seven faults and passes
# only if all are caught. Run under the gate lock: flock /srv/devops/build/clanky/.gate.lock scripts/gate-q4-blocks.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
# With CLANKY_BIN (clanky-gpui) the gate drives that binary and today's clanky is not built.
if [[ -z ${CLANKY_BIN:-} ]]; then
    flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
fi
if [[ ${1:-} == --faults ]]; then
    caught=0
    for fault in fail select alt naive idle classic osc; do
        if out=$(python3 scripts/gate-q4-blocks.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault ($(grep -m1 '^FAIL' <<<"$out"))"; caught=$((caught + 1))
        fi
    done
    [[ $caught == 7 ]] && { echo 'PASS: Q4 gate catches all 7 planted faults'; exit 0; }
    exit 1
fi
exec python3 scripts/gate-q4-blocks.py
