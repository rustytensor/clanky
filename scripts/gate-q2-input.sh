#!/usr/bin/env bash
# Q2 gate (command input): prints "PASS: Q2 command input" or fails. --faults plants six faults (seven with CLANKY_BIN,
# whose agent fault checks the agent picker) and passes only if all are caught. Run under the gate lock: flock /srv/devops/build/clanky/.gate.lock scripts/gate-q2-input.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
export CARGO_TARGET_DIR
# With CLANKY_BIN (clanky-gpui) the gate drives that binary and today's clanky is not needed.
if [[ -z ${CLANKY_BIN:-} ]]; then
    flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
fi
if [[ ${1:-} == --faults ]]; then
    caught=0
    faults=(colours ghost aside complete idle classic)
    [[ -z ${CLANKY_BIN:-} ]] || faults+=(agent)
    for fault in "${faults[@]}"; do
        if out=$(python3 scripts/gate-q2-input.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault ($(grep -m1 '^FAIL' <<<"$out"))"; caught=$((caught + 1))
        fi
    done
    [[ $caught == "${#faults[@]}" ]] && { echo "PASS: Q2 gate catches all $caught planted faults"; exit 0; }
    exit 1
fi
exec python3 scripts/gate-q2-input.py
