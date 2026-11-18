#!/usr/bin/env bash
# Editor pane gate: today's editor pane drawn by clanky-gpui. Prints one "ok:", "CAUGHT:", "pending:" or "FAIL:" line
# per check, then PASS or FAIL.
#
# Usage: CLANKY_GPUI_BIN=PATH scripts/gate-editor-pane.sh [--faults]
#   PATH is a clanky-gpui built with `--features control-faults` (planted faults need it; without a fault it behaves as
#   a release build).
#
# Without --faults:
#   1. today's editor gates, unchanged: editor-core-gate (clanky-edit) and clanky-panel-gate (the editor controller);
#   2. the window checks (editor-pane/check_editor_pane.py): open from the Files tree beside the focused pane, typing,
#      the pending-change marker, undo, Ctrl+S, the refused close with pending changes, no idle frames, syntax colours;
#   3. the controls gate on the main window with an editor pane open (its bar's Save and Close), at scales 1 and 1.5;
#   4. no loose numbers in the pane's sources (the design gate's check 1);
#   5. the engine check: clanky-edit, like the other engine crates, does not depend on gpui-ce;
#   6. speed in a 10,000-line file, only while the 1-minute load average is under 8; otherwise "pending".
# With --faults: each planted fault must fail the check aimed at it: save-not-written (save), marker-stays (save),
#   syntax-off-by-one (syntax), close-drops-pending (close), a loose radius (the design gate's fixture, loose-numbers)
#   and gpui-ce added to clanky-edit (engine-free).
set -euo pipefail
cd "$(dirname "$0")/.."

GATE_LOCK=${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}
BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
binary=${CLANKY_GPUI_BIN:-target/release/clanky-gpui}
WINDOW=scripts/editor-pane/check_editor_pane.py
ENGINE_CHECK=scripts/terminal-pane/check_engine_free.sh
LOAD_LIMIT=8
# fault:check — a planted fault and the window check that must fail because of it.
FAULTS=(save-not-written:save marker-stays:save syntax-off-by-one:syntax close-drops-pending:close)
failed=0

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# window ARGS...: the window checks under the gate lock, away from the desktop session; sets output and status.
window() {
  status=0
  output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$WINDOW" --app "$binary" "$@" 2>&1) || status=$?
}

if [[ ${1:-} == --faults ]]; then
  caught=0
  for entry in "${FAULTS[@]}"; do
    IFS=: read -r fault check <<<"$entry"
    window --fault "$fault"
    if ((status != 0)) && grep -q "^FAIL: $check " <<<"$output"; then
      echo "CAUGHT: $fault by $check ($(grep -m1 "^FAIL: $check " <<<"$output" | cut -c1-160))"
      caught=$((caught + 1))
    else
      fail "planted fault $fault was not caught by $check (exit $status): $(grep -m1 '^FAIL' <<<"$output" || true)"
    fi
  done
  if ! output=$(python3 scripts/editor-pane/check_loose_numbers.py scripts/window-chrome/fixtures/loose-literal.rs 2>&1) && [[ $output == *"1 loose numbers"* ]]; then
    echo "CAUGHT: a loose radius by loose-numbers ($output)"
    caught=$((caught + 1))
  else
    fail "the loose radius in fixtures/loose-literal.rs was not caught: $output"
  fi
  if output=$(bash "$ENGINE_CHECK" --fault clanky-edit 2>&1) && [[ $output == *"FAIL: engine-free clanky-edit depends on gpui"* ]]; then
    echo "CAUGHT: gpui-ce in clanky-edit by engine-free"
    caught=$((caught + 1))
  else
    fail "planted gpui-ce in clanky-edit was not caught: $output"
  fi
  if ((failed == 0 && caught == ${#FAULTS[@]} + 2)); then
    echo "PASS: the editor pane gate catches all $caught planted faults"
    exit 0
  fi
  echo "FAIL: the editor pane gate caught $caught of $((${#FAULTS[@]} + 2)) planted faults"
  exit 1
fi

# 1. Today's editor gates, built in debug as gate-m7-panel.sh builds them; the panel gate starts its own server from
# the clanky binary beside it, in a scratch folder under the target.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}
scratch=$(mktemp -d /srv/devops/build/editor-pane-gate.XXXXXX)
trap 'rm -rf -- "${scratch:?}"' EXIT
mkdir -p "$scratch"/{home,config,data,state,cache,runtime}
chmod g-s,u=rwx,go= "$scratch"/runtime
if flock "$BUILD_LOCK" cargo build -q --offline --locked -j 8 -p clanky --bin clanky --bin clanky-panel-gate -p clanky-edit --bin editor-core-gate 2>"$scratch/build.log"; then
  for gate in editor-core-gate clanky-panel-gate; do
    status=0
    output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS HOME="$scratch/home" XDG_CONFIG_HOME="$scratch/config" \
      XDG_DATA_HOME="$scratch/data" XDG_STATE_HOME="$scratch/state" XDG_CACHE_HOME="$scratch/cache" XDG_RUNTIME_DIR="$scratch/runtime" \
      "$CARGO_TARGET_DIR/debug/$gate" 2>&1) || status=$?
    if ((status == 0)); then ok "$gate: $(tail -n 1 <<<"$output")"; else fail "$gate: $(tail -n 2 <<<"$output" | tr '\n' ' ')"; fi
  done
else
  fail "building today's editor gates: $(tail -n 3 "$scratch/build.log" | tr '\n' ' ')"
fi

# 2. The window checks.
window
grep -E '^(ok|FAIL):' <<<"$output" || true
if ((status == 0)); then ok "window checks pass"; else fail "window checks"; fi

# 3. The controls gate with an editor pane open.
status=0
# Its own run folder: the controls gate's default one is shared with other runs of it.
output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS CLANKY_CONTROLS_WORK="${CLANKY_CONTROLS_WORK:-/srv/devops/build/clanky-editor-controls-run}" \
  flock "$GATE_LOCK" python3 scripts/window-chrome/gate-controls.py --editor --screens main-window-editor --app "$binary" --scales 1,1.5 2>&1) || status=$?
grep -E '^FAIL:' <<<"$output" || true
if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate: $(grep -c '^ok:' <<<"$output") checks pass with the editor's bar"; else fail "controls gate with the editor's bar"; fi

# 4. No loose numbers in the pane's sources.
for source in crates/clanky-gpui/src/editor_pane.rs crates/clanky-gpui/src/main_window/editors.rs; do
  status=0
  output=$(python3 scripts/editor-pane/check_loose_numbers.py "$source" 2>&1) || status=$?
  if ((status == 0)); then ok "$output"; else fail "$output"; fi
done

# 5. The engine check (clanky-edit among the engine crates).
if output=$(bash "$ENGINE_CHECK" 2>&1) && [[ $output == ok:* ]]; then ok "${output#ok: }"; else fail "engine check: $output"; fi

# 6. Speed, only on a quiet machine.
load=$(cut -d' ' -f1 /proc/loadavg)
if python3 -c "import sys; sys.exit(0 if float('$load') < $LOAD_LIMIT else 1)"; then
  window --speed
  grep -E '^(ok|FAIL): speed' <<<"$output" || true
  if ((status == 0)); then ok "speed in a 10,000-line file"; else fail "speed in a 10,000-line file"; fi
else
  echo "pending: speed (1-minute load average $load, the check runs under $LOAD_LIMIT)"
fi

if ((failed)); then
  echo "FAIL: editor pane"
  exit 1
fi
echo "PASS: editor pane"
