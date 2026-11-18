#!/usr/bin/env bash
# Window chrome gate (G2): the design token gate and the controls gate, retargeted to clanky-gpui, then today's
# behaviour gates for these parts run on clanky-gpui (q2-input, q4-blocks, c5-panes, mouse, m12-clicks' chrome steps).
# Prints one "ok:", "CAUGHT:" or "FAIL:" line per step, then PASS or FAIL.
# --faults needs no clanky-gpui binary and no display: both gates run on a simulated window (scripts/window-chrome/
#   simulated_window.py) and on the fixtures; the good runs must pass and each of eight planted faults must fail the check
#   aimed at it. Without --faults the gate stops with "FAIL: clanky-gpui writes no control record yet" until the window
#   code writes the record (scripts/window-chrome/RECORD.md); after that it runs both gates on the real window, with
#   CLANKY_GPUI_BIN=PATH or target/release/clanky-gpui, under the gate lock.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=parallel-jobs.sh
source scripts/parallel-jobs.sh

DIR=scripts/window-chrome
GATE_LOCK=${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}
FIXTURE_SCALES=(1 1.5)
FIXTURE_THEME=graphite-dark
GOOD_SCREENS=(main-window main-window-recent main-window-agents) # the good run, one process each
# gate:fault:check — a planted fault and the check that must fail because of it.
FAULTS=(
  "gate-controls.py:hit-larger:hit-box"
  "gate-controls.py:press-fires:release"
  "gate-controls.py:tab-skip:tab-order"
  "gate-controls.py:border-2px:border-width"
  "gate-controls.py:click-takes-focus:click-focus"
  "gate-controls.py:active-tab-grows:tab-stable"
  "gate-controls.py:selected-segment-hover:hover-colour"
  "gate-design-tokens.py:tab-too-tall:tab-height"
  "gate-design-tokens.py:loose-literal:loose-numbers"
  "gate-design-tokens.py:pane-rounded:pane-radius"
)
failed=0
scratch=$(mktemp -d /srv/devops/build/window-chrome-gate.XXXXXX)
trap 'rm -rf -- "${scratch:?}"' EXIT

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# queue_gate NAME GATE ARGS...: starts one gate script in simulation in the background; take_job NAME then sets output
# and status from it.
queue_gate() {
  local name=$1 gate=$2
  shift 2
  start_job "$name" python3 "$DIR/$gate" --simulate --scales "${JOB_SCALES:-$(IFS=,; echo "${FIXTURE_SCALES[*]}")}" --theme "$FIXTURE_THEME" "$@"
}

if [[ ${1:-} == --faults ]]; then
  # The committed fixtures are what the generator draws now.
  for scale in "${FIXTURE_SCALES[@]}"; do
    python3 "$DIR/draw_chrome_reference.py" --scale "$scale" --theme "$FIXTURE_THEME" --out-dir "$scratch"
    for file in "record-good-$scale.jsonl" "main-window-good-$scale.png"; do
      if cmp -s "$scratch/$file" "$DIR/fixtures/$file"; then ok "fixture $file is what the generator draws"; else fail "fixture $file is stale: run draw_chrome_reference.py"; fi
    done
  done
  JOB_DIR=$scratch
  good_jobs=()
  for scale in "${FIXTURE_SCALES[@]}"; do
    for screen in "${GOOD_SCREENS[@]}"; do
      good_jobs+=("controls-$scale-$screen")
      JOB_SCALES=$scale queue_gate "controls-$scale-$screen" gate-controls.py --screens "$screen"
    done
  done
  queue_gate design gate-design-tokens.py --sources "$DIR/fixtures/clean.rs"
  for index in "${!FAULTS[@]}"; do
    IFS=: read -r gate fault check <<<"${FAULTS[$index]}"
    sources=("--sources" "$DIR/fixtures/clean.rs")
    [[ $fault != loose-literal ]] || sources=() # that fault scans its own fixture
    queue_gate "fault-$index" "$gate" --fault "$fault" "${sources[@]}"
  done
  take_jobs "${good_jobs[@]}"
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the simulated window"; else fail "controls gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  take_job design
  if ((status == 0)) && [[ $output == *"PASS: design tokens"* ]]; then ok "design gate passes the simulated window and the clean source fixture"; else fail "design gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  caught=0
  for index in "${!FAULTS[@]}"; do
    IFS=: read -r gate fault check <<<"${FAULTS[$index]}"
    take_job "fault-$index"
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
# Today's behaviour gates on clanky-gpui, each with its own window on a private display, under the gate lock. Each
# names in its docstring the steps it runs on clanky-gpui and the ones moved to G3 or G4.
target=$(realpath -m "$(dirname "$binary")/..")
CLANKY_BIN=$(realpath "$binary")
export CLANKY_BIN CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
if [[ ! -x $target/release/clanky-agent-gate ]]; then
  CARGO_TARGET_DIR=$target flock "${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}" \
    cargo build -q --offline --locked --release -j 8 -p clanky --bin clanky-agent-gate
fi
# behaviour NAME PASS-TEXT COMMAND...: runs one behaviour gate; ok when it exits 0 and prints PASS-TEXT.
behaviour() {
  local name=$1 want=$2
  shift 2
  status=0
  output=$(env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS CARGO_TARGET_DIR="$target" flock "$GATE_LOCK" "$@" 2>&1) || status=$?
  if ((status == 0)) && [[ $output == *"$want"* ]]; then ok "$name: $want"; else fail "$name: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
}
behaviour q2-input 'PASS: Q2 command input' env -u DISPLAY scripts/gate-q2-input.sh
behaviour q4-blocks 'PASS: Q4' env -u DISPLAY scripts/gate-q4-blocks.sh
behaviour c5-panes 'PASS: C5' env -u DISPLAY scripts/gate-c5-panes.sh
behaviour mouse 'PASS: mouse' env -u DISPLAY scripts/gate-mouse.sh
behaviour m12-clicks 'second session' bash -c 'source scripts/private-display.sh && run_on_private_display 1400x900x24 python3 scripts/gate-m12-clicks.py'
if ((failed)); then
  echo "FAIL: window chrome"
  exit 1
fi
echo "PASS: window chrome"
