#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
require_all=0
case $# in
    0) ;;
    1) if [[ $1 == --require-all ]]; then require_all=1; else echo 'FAIL: usage: gate-rc.sh [--require-all]' >&2; exit 2; fi ;;
    *) echo 'FAIL: usage: gate-rc.sh [--require-all]' >&2; exit 2 ;;
esac
fault=${CLANKY_GATE_FAULT:-}
if [[ -n $fault && $fault != rc ]]; then echo 'FAIL: unknown RC fault' >&2; exit 2; fi
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/srv/devops/build/clanky-pkg/target}
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CLANKY_BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
export CARGO_BUILD_JOBS=8
export WGPU_BACKEND=vulkan
icd=${CLANKY_SOFTWARE_VULKAN_ICD:-/usr/share/vulkan/icd.d/lvp_icd.x86_64.json}
export VK_ICD_FILENAMES=$icd VK_DRIVER_FILES=$icd
mkdir -p /srv/devops/build/clanky-pkg "$CARGO_TARGET_DIR" || exit 1
scratch=$(mktemp -d /srv/devops/build/clanky-pkg/rc-gate.XXXXXXXX) || exit 1
trap 'rm -rf -- "$scratch"' EXIT
failed=0
if [[ ! -f $icd ]]; then
    echo "FAIL: preparation (software Vulkan driver not found: $icd)"
    failed=1
elif env -u CLANKY_GATE_FAULT flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked --workspace --bins -j 8 >"$scratch/build.log" 2>&1; then
    echo 'PASS: preparation'
else
    rc=$?
    detail=$(tail -n 1 "$scratch/build.log" | cut -c1-200)
    echo "FAIL: preparation (build exit $rc; ${detail:-no diagnostic})"
    failed=1
fi
run_gate() {
    local path=$1 mode=${2:-normal} rc=0
    if [[ ! -f $path ]]; then
        if [[ $fault == rc && $path == scripts/gate-docs.sh ]]; then
            echo "FAIL: $path (planted RC fault; script absent)"
            failed=1
        elif (( require_all )); then
            echo "FAIL: $path (missing)"
            failed=1
        else
            echo "SKIP: $path (missing)"
        fi
        return
    fi
    if [[ ! -x $path ]]; then
        echo "FAIL: $path (not executable)"
        failed=1
        return
    fi
    if [[ $path == scripts/gate-m6-masking.sh ]]; then
        # Masking fixtures must never be captured in a file or printed.
        env -u CLANKY_GATE_FAULT CLANKY_RC_NO_NESTED=1 timeout 1800 "$path" >/dev/null 2>&1 || rc=$?
    elif [[ $mode == milestone ]]; then
        env -u CLANKY_GATE_FAULT -u CLANKY_RC_NO_NESTED timeout 1800 "$path" >"$scratch/gate.log" 2>&1 || rc=$?
    else
        env -u CLANKY_GATE_FAULT CLANKY_RC_NO_NESTED=1 timeout 1800 "$path" >"$scratch/gate.log" 2>&1 || rc=$?
    fi
    if (( rc != 0 )); then
        if [[ $path == scripts/gate-m6-masking.sh ]]; then
            echo "FAIL: $path (exit $rc; masking output suppressed)"
        else
            detail=$(tail -n 8 "$scratch/gate.log" | tr '\n' ' ' | cut -c1-600)
            echo "FAIL: $path (exit $rc; ${detail:-no diagnostic})"
        fi
        failed=1
    elif [[ $fault == rc && $path == scripts/gate-docs.sh ]]; then
        echo "FAIL: $path (planted RC fault)"
        failed=1
    else
        echo "PASS: $path"
    fi
}
if [[ -f scripts/gate-m5.sh ]]; then
    run_gate scripts/gate-m5.sh milestone
else
    if (( require_all )); then
        echo 'FAIL: scripts/gate-m5.sh (missing; M4 fallback follows)'
        failed=1
    else
        echo 'SKIP: scripts/gate-m5.sh (missing; M4 fallback follows)'
    fi
    run_gate scripts/gate-m4.sh milestone
fi
for gate in scripts/gate-m6-masking.sh scripts/gate-m7-core.sh scripts/gate-m7-panel.sh \
    scripts/gate-m8-compose.sh scripts/gate-m9.sh scripts/gate-polish-window.sh \
    scripts/gate-polish-session.sh scripts/gate-linux-packaging.sh scripts/gate-docs.sh; do
    run_gate "$gate"
done
if (( failed )); then echo 'FAIL: release-candidate gate'; exit 1; fi
echo 'PASS: release-candidate gate'
