#!/usr/bin/env bash
# Floating screens gate (G3): the palette, the agent picker, the completion list, the find bar and the context menu.
# Prints one "ok:", "CAUGHT:" or "FAIL:" line per step, then PASS or FAIL.
# Without --faults, after the two gates it measures speed (speed.py --scene palette) and runs the engine check.
# --faults needs no clanky-gpui binary and no display: the controls gate and the design gate (window-chrome/) run on a
#   simulated window with the floating screens (floating/simulated_floating.py); the good runs must pass and each of
#   the planted faults must fail the check aimed at it. The speed verdict (floating/speed_verdict.py) is proven on
#   fixture results, one slow number each, and the engine check on a clanky-state that depends on gpui-ce. Without --faults the gate stops with "FAIL: clanky-gpui has no
#   floating screens yet" until the window code writes a `find-bar` screen in its control record; after that it runs
#   both gates on the real window, with CLANKY_GPUI_BIN=PATH or target/release/clanky-gpui, under the gate lock.
# --popover checks the one popover every floating screen uses, on the test popover of a clanky-gpui built with
#   `--features control-faults,test-popover` (CLANKY_GPUI_BIN=PATH): the controls gate's checks on both of its screens
#   (placement inside the window and clear of the opener, beside it and from the bottom-right corner; Esc closing it and
#   giving the keyboard back; Tab kept inside; arrows; no idle frames), then the planted faults popover-over-opener and
#   esc-loses-focus, each of which must fail the check aimed at it.
# --palette checks the command palette alone on a clanky-gpui built with `--features control-faults`
#   (CLANKY_GPUI_BIN=PATH): the controls gate's palette screen at both scales, the design gate's palette rows, its behaviour
#   (floating/palette_steps.py: today's q1 T3 and q2 K3 steps on the control record), then the planted fault
#   palette-no-run, which must fail the controls gate's filter check and the steps' P4.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=parallel-jobs.sh
source scripts/parallel-jobs.sh

CHROME=scripts/window-chrome
FLOATING=scripts/floating
GATE_LOCK=${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}
SCALES=1,1.5
THEMES=(graphite-dark graphite-light)
SCREENS=main-window,context-menu,palette,agent-picker,completion-list,find-bar
# gate:fault:screens:check — a planted fault, the screens it runs on, and the check that must fail because of it.
FAULTS=(
  "gate-controls.py:popover-over-opener:completion-list:popover"
  "gate-controls.py:esc-loses-focus:palette:focus-return"
  "gate-controls.py:idle-draw:completion-list:idle-frames"
  "gate-controls.py:tab-traps-completion:completion-list:tab-completes"
  "gate-design-tokens.py:palette-row-short:-:palette-row-height"
  "gate-design-tokens.py:palette-text-big:-:palette-row-cap-height"
  "gate-controls.py:find-count-off:find-bar:find-count"
  "gate-controls.py:find-mark-accent:find-bar:find-highlight"
  "gate-controls.py:shift-enter-next:find-bar:find-previous"
  "gate-controls.py:icon-off:find-bar:icon-centring"
  "gate-design-tokens.py:find-field-short:-:find-bar-field-height"
)
SPEED_RUNS=3
ENGINE_CHECK=scripts/terminal-pane/check_engine_free.sh
BASELINE=scripts/terminal-pane/baseline.json
failed=0
scratch=$(mktemp -d /srv/devops/build/floating-gate.XXXXXX)
trap 'rm -rf -- "${scratch:?}"' EXIT

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# queue_gate NAME GATE ARGS...: starts one gate script on the simulated floating window in the background; take_job NAME
# then sets output and status from it.
queue_gate() {
  local name=$1 gate=$2
  shift 2
  start_job "$name" python3 "$CHROME/$gate" --simulate --floating --scales "${JOB_SCALES:-$SCALES}" "$@"
}

if [[ ${1:-} == --faults ]]; then
  python3 "$FLOATING/write_fixtures.py" --out-dir "$scratch"
  for file in "$FLOATING"/fixtures/*; do
    if cmp -s "$scratch/$(basename "$file")" "$file"; then ok "fixture $(basename "$file") is what the generator draws"; else fail "fixture $(basename "$file") is stale: run write_fixtures.py"; fi
  done
  for theme in "${THEMES[@]}"; do
    for scale in 1 1.5; do
      if python3 "$FLOATING/expected_floating.py" --scale "$scale" --theme "$theme" >/dev/null; then ok "expected values for $theme at scale $scale"; else fail "expected values for $theme at scale $scale"; fi
    done
  done
  JOB_DIR=$scratch
  good_jobs=()
  IFS=, read -r -a good_screens <<<"$SCREENS"
  for scale in 1 1.5; do
    for screen in "${good_screens[@]}"; do
      good_jobs+=("controls-$scale-$screen")
      JOB_SCALES=$scale queue_gate "controls-$scale-$screen" gate-controls.py --screens "$screen"
    done
  done
  queue_gate design gate-design-tokens.py --sources "$CHROME/fixtures/clean.rs"
  for index in "${!FAULTS[@]}"; do
    IFS=: read -r gate fault screens check <<<"${FAULTS[$index]}"
    extra=(--fault "$fault")
    if [[ $gate == gate-controls.py ]]; then extra+=(--screens "$screens"); else extra+=(--sources "$CHROME/fixtures/clean.rs"); fi
    queue_gate "fault-$index" "$gate" "${extra[@]}"
  done
  take_jobs "${good_jobs[@]}"
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the simulated floating screens"; else fail "controls gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  take_job design
  if ((status == 0)) && [[ $output == *"PASS: design tokens"* ]]; then ok "design gate passes the simulated floating screens"; else fail "design gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  caught=0
  extra_faults=0
  for index in "${!FAULTS[@]}"; do
    IFS=: read -r gate fault screens check <<<"${FAULTS[$index]}"
    take_job "fault-$index"
    if ((status != 0)) && grep -q "^FAIL: $check " <<<"$output"; then
      echo "CAUGHT: $fault by $check ($gate)"
      caught=$((caught + 1))
    else
      fail "planted fault $fault was not caught by $check in $gate (exit $status)"
    fi
  done
  # The speed verdict: the good run passes, each slow number is named.
  if python3 "$FLOATING/speed_verdict.py" "$FLOATING/fixtures/speed-good.json" "$BASELINE" >/dev/null; then ok "speed verdict passes the good run"; else fail "speed verdict on the good run"; fi
  for slow in idle:idle_draws cat:cat_s key:key_p50_ms popover:popover_ms; do
    IFS=: read -r name number <<<"$slow"
    if verdict=$(python3 "$FLOATING/speed_verdict.py" "$FLOATING/fixtures/speed-slow-$name.json" "$BASELINE") || ! grep -q "over the limit: $number" <<<"$verdict"; then
      fail "speed verdict did not name $number for a slow $name: $verdict"
    else
      echo "CAUGHT: slow $name by the speed verdict ($number)"
      caught=$((caught + 1))
      extra_faults=$((extra_faults + 1))
    fi
  done
  # The engine check: clanky-state is free of gpui-ce, and a planted gpui-ce dependency is caught.
  if output=$(bash "$ENGINE_CHECK" 2>&1) && [[ $output == ok:* ]]; then ok "${output#ok: }"; else fail "engine check on the workspace: $output"; fi
  if output=$(bash "$ENGINE_CHECK" --fault clanky-state 2>&1) && [[ $output == *"FAIL: engine-free clanky-state depends on gpui"* ]]; then
    echo "CAUGHT: gpui-ce in clanky-state (check engine-free)"
    caught=$((caught + 1))
    extra_faults=$((extra_faults + 1))
  else
    fail "planted engine fault in clanky-state was not caught: $output"
  fi
  if ((failed == 0 && caught == ${#FAULTS[@]} + extra_faults)); then
    echo "PASS: floating gates catch all $caught planted faults and pass the good fixtures"
    exit 0
  fi
  echo "FAIL: floating gates caught $caught of $((${#FAULTS[@]} + extra_faults)) planted faults"
  exit 1
fi

if [[ ${1:-} == --popover ]]; then
  binary=${CLANKY_GPUI_BIN:-target/release/clanky-gpui}
  # run_real ARGS...: the controls gate on the real window's test popover; sets output and status.
  run_real() {
    status=0
    output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$CHROME/gate-controls.py" --test-popover --app "$binary" --scales "$SCALES" "$@" 2>&1) || status=$?
  }
  run_real --screens test-popover,test-popover-corner
  grep -E '^(ok|FAIL):' <<<"$output" || true
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the test popover"; else fail "controls gate on the test popover"; fi
  caught=0
  for entry in popover-over-opener:test-popover-corner:popover esc-loses-focus:test-popover:focus-return; do
    IFS=: read -r fault screen check <<<"$entry"
    run_real --screens "$screen" --fault "$fault"
    if ((status != 0)) && grep -q "^FAIL: $check " <<<"$output"; then
      echo "CAUGHT: $fault by $check (gate-controls.py on the test popover)"
      caught=$((caught + 1))
    else
      fail "planted fault $fault was not caught by $check on the test popover (exit $status)"
    fi
  done
  if ((failed == 0 && caught == 2)); then
    echo "PASS: the popover"
    exit 0
  fi
  echo "FAIL: the popover"
  exit 1
fi

if [[ ${1:-} == --palette ]]; then
  binary=${CLANKY_GPUI_BIN:-target/release/clanky-gpui}
  # in_lock CMD...: a window run under the gate lock, with no desktop session reachable.
  in_lock() { env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" "$@"; }
  status=0
  output=$(in_lock python3 "$CHROME/gate-controls.py" --floating --app "$binary" --scales "$SCALES" --screens palette 2>&1) || status=$?
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the palette"; else fail "controls gate on the palette: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  status=0
  output=$(in_lock python3 "$CHROME/gate-design-tokens.py" --floating --app "$binary" --screens palette 2>&1) || status=$?
  grep -E '^(ok|FAIL): palette' <<<"$output" || true
  if ((status == 0)); then ok "design gate passes the palette"; else fail "design gate on the palette: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  status=0
  output=$(in_lock python3 "$FLOATING/palette_steps.py" --app "$binary" 2>&1) || status=$?
  grep -E '^(ok|FAIL):' <<<"$output" || true
  if ((status == 0)); then ok "palette steps"; else fail "palette steps"; fi
  caught=0
  status=0
  output=$(in_lock python3 "$CHROME/gate-controls.py" --floating --app "$binary" --scales 1 --screens palette --fault palette-no-run 2>&1) || status=$?
  if ((status != 0)) && grep -q "^FAIL: filter " <<<"$output"; then echo "CAUGHT: palette-no-run by filter (gate-controls.py)"; caught=$((caught + 1)); else fail "planted fault palette-no-run was not caught by filter (exit $status)"; fi
  status=0
  output=$(in_lock python3 "$FLOATING/palette_steps.py" --app "$binary" --fault palette-no-run 2>&1) || status=$?
  if ((status != 0)) && grep -q "^FAIL: P4 " <<<"$output"; then echo "CAUGHT: palette-no-run by P4 (palette_steps.py)"; caught=$((caught + 1)); else fail "planted fault palette-no-run was not caught by P4 (exit $status)"; fi
  if ((failed == 0 && caught == 2)); then
    echo "PASS: the palette"
    exit 0
  fi
  echo "FAIL: the palette"
  exit 1
fi

if ! grep -rqs '"find-bar"' crates/clanky-gpui/src; then
  echo "FAIL: clanky-gpui has no floating screens yet"
  exit 1
fi
binary=${CLANKY_GPUI_BIN:-target/release/clanky-gpui}
for gate in gate-controls.py gate-design-tokens.py; do
  status=0
  args=(--floating --app "$binary")
  [[ $gate != gate-controls.py ]] || args+=(--screens "$SCREENS")
  output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$CHROME/$gate" "${args[@]}" 2>&1) || status=$?
  if ((status == 0)); then ok "$gate: $(tail -n 1 <<<"$output")"; else fail "$gate: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
done
# Check 4, the palette's part: today's q1 T3 and q2 K3 steps on the control record.
status=0
output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$FLOATING/palette_steps.py" --app "$binary" 2>&1) || status=$?
if ((status == 0)); then ok "palette steps: $(tail -n 1 <<<"$output")"; else fail "palette steps: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
# Check 5: speed with two tabs, four panes and the palette open (the old window's numbers are in baseline.json).
# CLANKY_GATE_SPEED_PENDING=1 leaves it out when the machine is too busy to measure, as G1's gate does; the gate then
# ends in "PASS: floating screens, speed pending", never a plain PASS.
speed_pending=0
if [[ ${CLANKY_GATE_SPEED_PENDING:-} == 1 ]]; then
  speed_pending=1
  echo "pending: speed (check 5) left out, load $(cut -d' ' -f1 /proc/loadavg)"
elif bash scripts/terminal-pane/speed.sh "$binary" --scene palette --runs "$SPEED_RUNS" --out "$scratch/speed.json" >"$scratch/speed.log" 2>&1; then
  if verdict=$(python3 "$FLOATING/speed_verdict.py" "$scratch/speed.json" "$BASELINE"); then ok "speed: $verdict"; else fail "speed: $verdict"; fi
else
  fail "speed harness failed: $(tail -n 1 "$scratch/speed.log")"
fi
# Check 6: clanky-state is free of gpui-ce.
if output=$(bash "$ENGINE_CHECK" 2>&1) && [[ $output == ok:* ]]; then ok "${output#ok: }"; else fail "engine check: $output"; fi
if ((failed)); then
  echo "FAIL: floating screens"
  exit 1
fi
if ((speed_pending)); then
  echo "PASS: floating screens, speed pending"
else
  echo "PASS: floating screens"
fi
