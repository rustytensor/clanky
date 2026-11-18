#!/usr/bin/env bash
# Menu gate: prints "PASS: menu" or fails. --faults plants five faults and passes only if all are caught.
# With CLANKY_BIN (clanky-gpui built with the control-faults feature) it drives that window instead, and --faults
# plants a sixth: the window drawing its menu over the pointer that opened it.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
export CARGO_TARGET_DIR
# With CLANKY_BIN the window under test is already built; today's clanky is not.
[[ -n ${CLANKY_BIN:-} ]] || flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" cargo build -q --offline --locked -j 8 -p clanky --bin clanky
faults=(copy shift link reset postai)
[[ -z ${CLANKY_BIN:-} ]] || faults+=(over)
if [[ ${1:-} == --faults ]]; then
    caught=0
    for fault in "${faults[@]}"; do
        if out=$(python3 scripts/gate-menu.py --fault "$fault"); then
            echo "FAIL: planted fault $fault was not caught"
        else
            echo "CAUGHT: $fault ($(grep -m1 '^FAIL' <<<"$out"))"; caught=$((caught + 1))
        fi
    done
    [[ $caught == "${#faults[@]}" ]] && { echo "PASS: menu gate catches all ${#faults[@]} planted faults"; exit 0; }
    exit 1
fi
exec python3 scripts/gate-menu.py
