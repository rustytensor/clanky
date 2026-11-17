#!/usr/bin/env bash
# C5 gate ("New windows open with" One pane | Two panes): prints "PASS: C5 new windows open with one or two panes"
# or fails. --faults plants five faults (four on clanky-gpui) and passes only if each is caught by the check it targets.
# Run under the gate lock: flock /srv/devops/build/clanky/.gate.lock scripts/gate-c5-panes.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
# With CLANKY_BIN (clanky-gpui) the gate drives that binary and today's clanky is not needed.
if [[ -z ${CLANKY_BIN:-} ]]; then
    flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
fi
if [[ ${1:-} == --faults ]]; then
    declare -A want=([nokey]=P2 [noclick]=S1 [badvalue]=E1 [norestore]=R1 [idle]=I1)
    faults=(nokey noclick badvalue norestore idle)
    # clanky-gpui runs P1, P2, E1's pane count, R1 and the idle checks; the Settings ones come in G4.
    [[ -n ${CLANKY_BIN:-} ]] && faults=(nokey badvalue norestore idle)
    caught=0
    for fault in "${faults[@]}"; do
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
    [[ $caught == "${#faults[@]}" ]] && { echo "PASS: C5 gate catches all ${#faults[@]} planted faults"; exit 0; }
    exit 1
fi
exec python3 scripts/gate-c5-panes.py "$@"
