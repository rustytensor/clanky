#!/usr/bin/env bash
# C5 gate ("New windows open with" One pane | Two panes): prints "PASS: C5 new windows open with one or two panes"
# or fails. --faults plants five faults and passes only if each is caught by the check it targets.
# Run under the gate lock: flock /srv/devops/build/clanky/.gate.lock scripts/gate-c5-panes.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
if [[ ${1:-} == --faults ]]; then
    declare -A want=([nokey]=P2 [noclick]=S1 [badvalue]=E1 [norestore]=R1 [idle]=I1)
    caught=0
    for fault in nokey noclick badvalue norestore idle; do
        if out=$(python3 scripts/gate-c5-panes.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            line=$(grep -m1 '^FAIL' <<<"$out" || true)
            if [[ $line == "FAIL: C5 ${want[$fault]} "* ]]; then
                echo "CAUGHT: $fault ($line)"; caught=$((caught + 1))
            else
                echo "FAIL: planted fault $fault failed elsewhere, not at ${want[$fault]}: $line"
            fi
        fi
    done
    [[ $caught == 5 ]] && { echo 'PASS: C5 gate catches all 5 planted faults'; exit 0; }
    exit 1
fi
exec python3 scripts/gate-c5-panes.py "$@"
