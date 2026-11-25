#!/usr/bin/env bash
# Settings gate: every page of the Settings window, its look, its controls and the config file behind them.
# Prints one "ok:", "CAUGHT:" or "FAIL:" line per step, then PASS or FAIL.
# Without --faults, after the two gates it measures speed (speed.py --scene settings) and runs the engine check.
# --faults needs no clanky-gpui binary and no display: the controls gate and the design gate (window-chrome/) run on a
#   simulated window with the Settings pages (settings/simulated_settings.py); the good runs must pass and each of
#   the planted faults must fail the check aimed at it. The speed verdict (floating/speed_verdict.py) is proven on
#   fixture results, one slow number each, and the engine check on a clanky-state that depends on gpui-ce. Without
#   --faults the gate stops with "FAIL: clanky-gpui has no Settings yet" until the window code writes an
#   `error-banner` landmark in its control record; after that it runs both gates on the real window, with
#   CLANKY_GPUI_BIN=PATH or target/release/clanky-gpui, under the gate lock.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=parallel-jobs.sh
source scripts/parallel-jobs.sh

CHROME=scripts/window-chrome
FLOATING=scripts/floating
SETTINGS=scripts/settings
# Scratch folders and the lock go in the system temp folder unless CLANKY_GATE_TMP names another.
GATE_TMP=${CLANKY_GATE_TMP:-${TMPDIR:-/tmp}}
GATE_LOCK=${CLANKY_GATE_LOCK:-$GATE_TMP/clanky-gate.lock}
SCALES=1,1.5
THEMES=(graphite-dark graphite-light)
SCREENS=settings-general,settings-appearance,settings-terminal,settings-editor,settings-agents,settings-history,settings-keys
# gate:fault:screens:check — a planted fault, the screens it runs on, and the check that must fail because of it.
FAULTS=(
  "gate-design-tokens.py:card-padding:-:settings-card-padding-x"
  "gate-controls.py:setting-not-written:settings-general:config-roundtrip"
  "gate-controls.py:switch-hit-larger:settings-general:hit-box"
  "gate-controls.py:page-control-out-of-tab-order:settings-general:tab-order"
)
SPEED_RUNS=3
ENGINE_CHECK=scripts/terminal-pane/check_engine_free.sh
BASELINE=scripts/terminal-pane/baseline.json
failed=0
scratch=$(mktemp -d "$GATE_TMP/settings-gate.XXXXXX")
trap 'rm -rf -- "${scratch:?}"' EXIT

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# queue_gate NAME GATE ARGS...: starts one gate script on the simulated Settings window in the background; take_job NAME
# then sets output and status from it.
queue_gate() {
  local name=$1 gate=$2
  shift 2
  start_job "$name" python3 "$CHROME/$gate" --simulate --settings --scales "${JOB_SCALES:-$SCALES}" "$@"
}

if [[ ${1:-} == --faults ]]; then
  python3 "$SETTINGS/write_fixtures.py" --out-dir "$scratch"
  for file in "$SETTINGS"/fixtures/*; do
    if cmp -s "$scratch/$(basename "$file")" "$file"; then ok "fixture $(basename "$file") is what the generator draws"; else fail "fixture $(basename "$file") is stale: run write_fixtures.py"; fi
  done
  for theme in "${THEMES[@]}"; do
    for scale in 1 1.5; do
      if python3 "$SETTINGS/expected_settings.py" --scale "$scale" --theme "$theme" >/dev/null; then ok "expected values for $theme at scale $scale"; else fail "expected values for $theme at scale $scale"; fi
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
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the simulated Settings pages"; else fail "controls gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  take_job design
  if ((status == 0)) && [[ $output == *"PASS: design tokens"* ]]; then ok "design gate passes the simulated Settings window"; else fail "design gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
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
  if python3 "$FLOATING/speed_verdict.py" "$SETTINGS/fixtures/speed-settings-good.json" "$BASELINE" >/dev/null; then ok "speed verdict passes the good run"; else fail "speed verdict on the good run"; fi
  for slow in idle:idle_draws cat:cat_s key:key_p50_ms popover:popover_ms; do
    IFS=: read -r name number <<<"$slow"
    if verdict=$(python3 "$FLOATING/speed_verdict.py" "$SETTINGS/fixtures/speed-settings-slow-$name.json" "$BASELINE") || ! grep -q "over the limit: $number" <<<"$verdict"; then
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
    echo "PASS: settings gates catch all $caught planted faults and pass the good fixtures"
    exit 0
  fi
  echo "FAIL: settings gates caught $caught of $((${#FAULTS[@]} + extra_faults)) planted faults"
  exit 1
fi

if ! grep -rqs '"error-banner"' crates/clanky-gpui/src; then
  echo "FAIL: clanky-gpui has no Settings yet"
  exit 1
fi
binary=${CLANKY_GPUI_BIN:-target/release/clanky-gpui}
# The controls gate runs one page at one scale per window: one window driven through every page slows down
# until a run takes hours.
IFS=, read -r -a screens <<<"$SCREENS"
for scale in 1 1.5; do
  for screen in "${screens[@]}"; do
    status=0
    output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$CHROME/gate-controls.py" --settings --app "$binary" --scales "$scale" --screens "$screen" 2>&1) || status=$?
    if ((status == 0)); then ok "gate-controls.py $screen @$scale: $(tail -n 1 <<<"$output")"; else fail "gate-controls.py $screen @$scale: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  done
done
status=0
output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$CHROME/gate-design-tokens.py" --settings --app "$binary" 2>&1) || status=$?
if ((status == 0)); then ok "gate-design-tokens.py: $(tail -n 1 <<<"$output")"; else fail "gate-design-tokens.py: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
# Speed with two tabs, four panes and Settings open (the old window's numbers are in baseline.json).
if bash scripts/terminal-pane/speed.sh "$binary" --scene settings --runs "$SPEED_RUNS" --out "$scratch/speed.json" >"$scratch/speed.log" 2>&1; then
  if verdict=$(python3 "$FLOATING/speed_verdict.py" "$scratch/speed.json" "$BASELINE"); then ok "speed: $verdict"; else fail "speed: $verdict"; fi
else
  fail "speed harness failed: $(tail -n 1 "$scratch/speed.log")"
fi
# clanky-state is free of gpui-ce.
if output=$(bash "$ENGINE_CHECK" 2>&1) && [[ $output == ok:* ]]; then ok "${output#ok: }"; else fail "engine check: $output"; fi
if ((failed)); then
  echo "FAIL: Settings"
  exit 1
fi
echo "PASS: Settings"
