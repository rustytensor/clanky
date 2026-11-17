#!/usr/bin/env bash
# Floating screens gate (G3): the palette, the agent picker, the completion list, the find bar and the context menu.
# Prints one "ok:", "CAUGHT:" or "FAIL:" line per step, then PASS or FAIL.
# --faults needs no clanky-gpui binary and no display: the controls gate and the design gate (window-chrome/) run on a
#   simulated window with the floating screens (floating/simulated_floating.py); the good runs must pass and each of
#   four planted faults must fail the check aimed at it. Without --faults the gate stops with "FAIL: clanky-gpui has no
#   floating screens yet" until the window code writes a `find-bar` screen in its control record; after that it runs
#   both gates on the real window, with CLANKY_GPUI_BIN=PATH or target/release/clanky-gpui, under the gate lock.
set -euo pipefail
cd "$(dirname "$0")/.."

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
  "gate-design-tokens.py:palette-row-short:-:palette-row-height"
)
failed=0
scratch=$(mktemp -d /srv/devops/build/floating-gate.XXXXXX)
trap 'rm -rf -- "${scratch:?}"' EXIT

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# run_gate GATE ARGS...: runs one gate script on the simulated floating window; sets output and status.
run_gate() {
  local gate=$1
  shift
  status=0
  output=$(python3 "$CHROME/$gate" --simulate --floating --scales "$SCALES" "$@" 2>&1) || status=$?
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
  run_gate gate-controls.py --screens "$SCREENS"
  if ((status == 0)) && [[ $output == *"PASS: controls"* ]]; then ok "controls gate passes the simulated floating screens"; else fail "controls gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  run_gate gate-design-tokens.py --sources "$CHROME/fixtures/clean.rs"
  if ((status == 0)) && [[ $output == *"PASS: design tokens"* ]]; then ok "design gate passes the simulated floating screens"; else fail "design gate on the good window: $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"; fi
  caught=0
  for entry in "${FAULTS[@]}"; do
    IFS=: read -r gate fault screens check <<<"$entry"
    extra=(--fault "$fault")
    if [[ $gate == gate-controls.py ]]; then extra+=(--screens "$screens"); else extra+=(--sources "$CHROME/fixtures/clean.rs"); fi
    run_gate "$gate" "${extra[@]}"
    if ((status != 0)) && grep -q "^FAIL: $check " <<<"$output"; then
      echo "CAUGHT: $fault by $check ($gate)"
      caught=$((caught + 1))
    else
      fail "planted fault $fault was not caught by $check in $gate (exit $status)"
    fi
  done
  if ((failed == 0 && caught == ${#FAULTS[@]})); then
    echo "PASS: floating gates catch all $caught planted faults and pass the good fixtures"
    exit 0
  fi
  echo "FAIL: floating gates caught $caught of ${#FAULTS[@]} planted faults"
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
if ((failed)); then
  echo "FAIL: floating screens"
  exit 1
fi
echo "PASS: floating screens"
