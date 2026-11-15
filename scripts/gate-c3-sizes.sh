#!/usr/bin/env bash
# C3 gate (comfortable sizes, HiDPI, the look and feel of Ptyxis): prints "PASS: C3 sizes" or fails.
# --faults plants seven faults and passes only if each is caught by the check it targets.
# --live runs the separate DP-2 check (native Wayland, scale 1.25) under /srv/devops/scripts/dp2-lock.
# Run under the gate lock: flock /srv/devops/build/clanky/.gate.lock scripts/gate-c3-sizes.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
if [[ ${1:-} == --live ]]; then
    exec /srv/devops/scripts/dp2-lock python3 scripts/gate-c3-sizes.py --live
fi
if [[ ${1:-} == --faults ]]; then
    declare -A want=([upscale]=X1 [line]=L1 [idle]=I1 [font]=C1 [noscroll]=S1 [nohover]=B1 [scale]=D1)
    caught=0
    for fault in upscale line idle font noscroll nohover scale; do
        if out=$(python3 scripts/gate-c3-sizes.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            line=$(grep -m1 '^FAIL' <<<"$out" || true)
            if [[ $line == "FAIL: C3 ${want[$fault]} "* ]]; then
                echo "CAUGHT: $fault ($line)"; caught=$((caught + 1))
            else
                echo "FAIL: planted fault $fault failed elsewhere, not at ${want[$fault]}: $line"
            fi
        fi
    done
    [[ $caught == 7 ]] && { echo 'PASS: C3 gate catches all 7 planted faults'; exit 0; }
    exit 1
fi
exec python3 scripts/gate-c3-sizes.py "$@"
