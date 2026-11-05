#!/usr/bin/env bash
# M12 integration gate; incomplete checks are explicitly fail-closed.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/srv/devops/build/clanky-pkg/target}
export CLANKY_BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
export VK_DRIVER_FILES=${CLANKY_SOFTWARE_VULKAN_ICD:-/usr/share/vulkan/icd.d/lvp_icd.x86_64.json}
export VK_ICD_FILENAMES=$VK_DRIVER_FILES LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=vulkan LP_NUM_THREADS=8
case ${CLANKY_GATE_FAULT:-} in ''|look|click|idle|robot) ;; *) echo 'FAIL: unknown M12 fault' >&2; exit 1;; esac
mkdir -p "$CARGO_TARGET_DIR/look" || exit 1
work=$(mktemp -d "$CARGO_TARGET_DIR/look/m12.XXXXXXXX") || exit 1
trap 'rm -rf -- "$work"' EXIT
failed=0
run_step() {
    local name=$1; shift
    if "$@"; then echo "PASS: $name"; else local rc=$?; echo "FAIL: $name (exit $rc)" >&2; failed=1; fi
}
if ! flock "$CLANKY_BUILD_LOCK" cargo build --offline --locked -p clanky \
    --bin clanky-calm-gate --bin clanky-settings-gate --bin clanky-look-gate --bin clanky --bin clanky-agent-gate -j 8; then
    echo 'FAIL: M12 build' >&2; exit 1
fi
run_step 'idle scheduler' "$CARGO_TARGET_DIR/debug/clanky-look-gate" --idle-only
run_step 'real window idle and key echo' xvfb-run -a -s '-screen 0 1400x900x24 -nolisten tcp' python3 scripts/gate-m12-idle.py
run_step 'real pointer checks (partial)' xvfb-run -a -s '-screen 0 1400x900x24 -nolisten tcp' python3 scripts/gate-m12-clicks.py
if [[ ${CLANKY_GATE_FAULT:-} == click ]]; then
    run_step 'additional Settings clicks' env CLANKY_GATE_BINARIES_READY=1 scripts/check-settings-clicks.sh
else
    run_step 'additional Settings clicks' env -u CLANKY_GATE_FAULT CLANKY_GATE_BINARIES_READY=1 scripts/check-settings-clicks.sh
fi
if [[ ${CLANKY_GATE_FAULT:-} == robot ]]; then
    run_step 'robot offscreen' env CLANKY_ROBOT_FAULT=1 flock "$CLANKY_BUILD_LOCK" cargo test --offline --locked -p clanky --lib robot::tests::robot_frames -- --exact --nocapture
else
    run_step 'robot offscreen' env -u CLANKY_ROBOT_FAULT flock "$CLANKY_BUILD_LOCK" cargo test --offline --locked -p clanky --lib robot::tests::robot_frames -- --exact --nocapture
fi
run_step 'robot first-run marker' flock "$CLANKY_BUILD_LOCK" cargo test --offline --locked -p clanky --lib robot::tests::startup_marker -- --exact
if [[ ${CLANKY_GATE_FAULT:-} == robot ]]; then
    run_step 'robot real window' env CLANKY_GATE_BINARIES_READY=1 scripts/check-robot-window.sh
else
    run_step 'robot real window' env -u CLANKY_GATE_FAULT CLANKY_GATE_BINARIES_READY=1 scripts/check-robot-window.sh
fi
run_step 'built-in themes and Catppuccin' flock "$CLANKY_BUILD_LOCK" cargo test --offline --locked -p clanky --lib themes::tests::builtin_documents -- --exact --nocapture
run_step 'theme contrast and storage' scripts/gate-m12-themes.sh
run_step 'Post-AI server' python3 scripts/gate-m12-post-ai.py
if "$CARGO_TARGET_DIR/debug/clanky-settings-gate" "$work" > "$work/settings.json"; then
    run_step 'Settings geometry, pixels and labels' python3 scripts/gate-m12-settings.py < "$work/settings.json"
else
    echo 'FAIL: M12 Settings observations' >&2; failed=1
fi
if "$CARGO_TARGET_DIR/debug/clanky-calm-gate" "$work" > "$work/observations.json"; then
    run_step 'layout, PTYs, restore, pixels and text floor' python3 scripts/gate-m12-calm.py --no-idle < "$work/observations.json"
    run_step 'theme pictures' python3 scripts/m12-pictures.py "$work" "$CARGO_TARGET_DIR/look"
else
    echo 'FAIL: M12 scene observations' >&2; failed=1
fi
# The calm scene has no command-card action hit target yet; neither click
# fixture proves real-window block hover and clipboard copy.
echo 'FAIL: M12 block hover/copy check is not implemented' >&2
failed=1
if [[ ${CLANKY_RC_NO_NESTED:-0} != 1 ]]; then
    run_step 'release regression' env -u CLANKY_GATE_FAULT -u CLANKY_RC_NO_NESTED CLANKY_M12_PARENT=1 scripts/gate-rc.sh
else
    echo 'SKIP: release regression (already running under gate-rc)'
fi
if ((failed)); then echo 'FAIL: M12 calm' >&2; exit 1; fi
echo 'PASS: M12 calm'
