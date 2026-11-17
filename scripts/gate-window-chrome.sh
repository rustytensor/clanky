#!/usr/bin/env bash
# Window chrome gate (G2): the design token gate and the controls gate, retargeted to clanky-gpui.
# Prints one "ok:", "CAUGHT:" or "FAIL:" line per step, then PASS or FAIL.
# --faults needs no clanky-gpui binary and no display: both gates run on a simulated window (scripts/window-chrome/
#   simulated_window.py) and on the fixtures; the good runs must pass and each of six planted faults must fail the check
#   aimed at it. Without --faults the gate stops with "FAIL: clanky-gpui writes no control record yet" until the window
#   code writes the record (scripts/window-chrome/RECORD.md); after that it runs both gates on the real window, with
#   CLANKY_GPUI_BIN=PATH or target/release/clanky-gpui, under the gate lock.
set -euo pipefail
cd "$(dirname "$0")/.."

DIR=scripts/window-chrome
GATE_LOCK=${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}
FIXTURE_SCALES=(1 1.5)
FIXTURE_THEME=graphite-dark
# gate:fault:check — a planted fault and the check that must fail because of it.
FAULTS=(
  "gate-controls.py:hit-larger:hit-box"
  "gate-controls.py:press-fires:release"
  "gate-controls.py:tab-skip:tab-order"
  "gate-controls.py:border-2px:border-width"
  "gate-design-tokens.py:tab-too-tall:tab-height"
  "gate-design-tokens.py:loose-literal:loose-numbers"
)
failed=0
scratch=$(mktemp -d /srv/devops/build/window-chrome-gate.XXXXXX)
trap 'rm -rf -- "${scratch:?}"' EXIT

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# run_gate GATE ARGS...: runs one gate script in simulation; sets output and status.
run_gate() {
  local gate=$1
  shift
  status=0
  output=$(python3 "$DIR/$gate" --simulate --scales "$(IFS=,; echo "${FIXTURE_SCALES[*]}")" --theme "$FIXTURE_THEME" "$@" 2>&1) || status=$?
}

if [[ ${1:-} == --faults ]]; then
  # The committed fixtures are what the generator draws now.
  for scale in "${FIXTURE_SCALES[@]}"; do
    python3 "$DIR/draw_chrome_reference.py" --scale "$scale" --theme "$FIXTURE_THEME" --out-dir "$scratch"
    for file in "record-good-$scale.jsonl" "main-window-good-$scale.png"; do
      if cmp -s "$scratch/$file" "$DIR/fixtures/$file"; then ok "fixture $file is what the generator draws"; else fail "fixture $file is stale: run draw_chrome_reference.py"; fi
    done
  done
  run_gate gate-controls.py
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the simulated window"; else fail "controls gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  run_gate gate-design-tokens.py --sources "$DIR/fixtures/clean.rs"
  if ((status == 0)) && [[ $output == *"PASS: design tokens"* ]]; then ok "design gate passes the simulated window and the clean source fixture"; else fail "design gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  caught=0
  for entry in "${FAULTS[@]}"; do
    IFS=: read -r gate fault check <<<"$entry"
    sources=("--sources" "$DIR/fixtures/clean.rs")
    [[ $fault != loose-literal ]] || sources=()   # that fault scans its own fixture
    run_gate "$gate" --fault "$fault" "${sources[@]}"
    if ((status != 0)) && grep -q "^FAIL: $check " <<<"$output"; then
      echo "CAUGHT: $fault by $check ($gate)"
      caught=$((caught + 1))
    else
      fail "planted fault $fault was not caught by $check in $gate (exit $status)"
    fi
  done
  if ((failed == 0 && caught == ${#FAULTS[@]})); then
    echo "PASS: window chrome gates catch all $caught planted faults and pass the good fixtures"
    exit 0
  fi
  echo "FAIL: window chrome gates caught $caught of ${#FAULTS[@]} planted faults"
  exit 1
fi

if ! grep -rqs CLANKY_CONTROL_RECORD crates/clanky-gpui/src; then
  echo "FAIL: clanky-gpui writes no control record yet"
  exit 1
fi
binary=${CLANKY_GPUI_BIN:-target/release/clanky-gpui}
for gate in gate-controls.py gate-design-tokens.py; do
  status=0
  output=$(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS flock "$GATE_LOCK" python3 "$DIR/$gate" --app "$binary" 2>&1) || status=$?
  if ((status == 0)); then ok "$gate: $(tail -n 1 <<<"$output")"; else fail "$gate: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
done
if ((failed)); then
  echo "FAIL: window chrome"
  exit 1
fi
echo "PASS: window chrome"
