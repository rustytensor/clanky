#!/usr/bin/env bash
# M12 is incomplete: a partial scene cannot pass the milestone gate.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/srv/devops/build/clanky-pkg/target}
export CLANKY_BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
export VK_DRIVER_FILES=${CLANKY_SOFTWARE_VULKAN_ICD:-/usr/share/vulkan/icd.d/lvp_icd.x86_64.json}
export VK_ICD_FILENAMES=$VK_DRIVER_FILES LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=vulkan
case ${CLANKY_GATE_FAULT:-} in ''|look|click|idle|robot) ;; *) echo 'FAIL: unknown M12 fault' >&2; exit 1;; esac
mkdir -p "$CARGO_TARGET_DIR/look"
work=$(mktemp -d "$CARGO_TARGET_DIR/look/m12.XXXXXXXX")
trap 'rc=$?; rm -rf -- "$work"; if ((rc != 0)); then echo "FAIL: M12 calm" >&2; fi' EXIT
flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky --bin clanky-calm-gate -j 8
"$CARGO_TARGET_DIR/debug/clanky-calm-gate" "$work" > "$work/observations.json"
python3 scripts/gate-m12-calm.py --scene-only < "$work/observations.json"
echo 'FAIL: M12 gate incomplete: PTYs, persistence, clicks, idle, themes, Post-AI API, robot, pictures and regression are not yet checked' >&2
exit 1
