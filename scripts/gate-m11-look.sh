#!/usr/bin/env bash
set -euo pipefail
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: M11 look" >&2; fi' EXIT
cd "$(dirname "$0")/.."
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CARGO_TARGET_DIR=/srv/devops/build/clanky-pkg/target
export CLANKY_BUILD_LOCK=/srv/devops/build/clanky/.build.lock
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json
export VK_ICD_FILENAMES=$VK_DRIVER_FILES
export LIBGL_ALWAYS_SOFTWARE=1
export WGPU_BACKEND=vulkan
case ${CLANKY_GATE_FAULT:-} in ''|look) ;; *) echo 'FAIL: unknown look fault' >&2; exit 1;; esac
mkdir -p "$CARGO_TARGET_DIR/look"
work=$(mktemp -d "$CARGO_TARGET_DIR/look/gate.XXXXXXXX")
trap 'rc=$?; rm -rf -- "$work"; if (( rc != 0 )); then echo "FAIL: M11 look" >&2; fi' EXIT
flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky --bin clanky-look-gate -j 8
"$CARGO_TARGET_DIR/debug/clanky-look-gate" --picture "$work/scene.png" > "$work/observations.json"
python3 scripts/gate-m11-look.py < "$work/observations.json"
[[ -s $work/scene.png ]] || { echo 'FAIL: look picture missing' >&2; exit 1; }
mv -- "$work/scene.png" "$CARGO_TARGET_DIR/look/scene.png"
if [[ ${CLANKY_RC_NO_NESTED:-0} != 1 ]]; then
    env -u CLANKY_GATE_FAULT CLANKY_M11_PARENT=1 scripts/gate-rc.sh
fi
echo 'PASS: M11 look'
