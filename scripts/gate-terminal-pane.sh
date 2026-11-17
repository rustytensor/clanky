#!/usr/bin/env bash
# Terminal pane gate: one terminal pane on gpui-ce. Prints one "ok: ..." or "FAIL: ..." line per step, then "PASS: terminal pane"
# only if every step printed ok. --faults needs no clanky-gpui binary: it runs the screenshot checks against drawn
# reference images with five planted faults at both scales, and the engine check with its planted fault, and passes
# only if each fault is caught by the check aimed at it.
# CLANKY_GPUI_BIN=PATH skips the build and uses that binary (for trying the script against a stand-in).
# CLANKY_TODAY_BIN=PATH (or --today-bin PATH) names today's clanky for the speed comparison; without it the gate
# builds it from the c-int worktree.
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-$PWD/target}")
export CARGO_TARGET_DIR
# The same ssh fetch settings as check_engine_free.sh: our copy of gpui-ce lives on git.servorra.com.
export CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0=url.git@git.servorra.com:.insteadOf GIT_CONFIG_VALUE_0=https://git.servorra.com/

CHECK_DIR=scripts/terminal-pane
TODAY_WORKTREE=/srv/devops/build/clanky-c-int
BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
GATE_LOCK=${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}
SCALES=(1 1.5)
WINDOW_SIZE=2000x900        # logical pixels, the launch contract's --size
DISPLAY_SIZE=3200x1400x24   # fits the larger window at scale 1.5 (3000x1350)
WAIT_SECONDS=20             # the window and row 10's ink must appear within this long
FIXTURE_COMMAND="clear; bash $PWD/$CHECK_DIR/fixture.sh"
SPEED_RUNS=3                # alternating runs of each binary
SPEED_LIMIT_TIME=1.1        # check 6: cat of 100 MB at most this multiple of today's
SPEED_LIMIT_KEY_MS=2        # key to screen p50 and p99 at most today's plus this
SPEED_LIMIT_START=1.2       # start to first frame at most this multiple of today's
FAULT_PAIRS=(drift:placement background:colours nounderline:attributes boxgap:wide halfblock:wide)

today_binary=${CLANKY_TODAY_BIN:-}
if [[ ${1:-} == --today-bin ]]; then
  today_binary=${2:?--today-bin needs a path}
  shift 2
fi

scratch=$(mktemp -d /srv/devops/build/terminal-pane-gate.XXXXXX)
# Nothing this gate starts may reach the user's desktop session: every display goes to a private Xvfb.
unset WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS
pids=()
failed=0
# The windows' session server lives in the scratch folder, never in the user's runtime folder: a gate must not reach
# Marc's own running Clanky server or his config.
server_socket=$scratch/server.sock
SHOTS_DIR=${CLANKY_GATE_SHOTS:-}   # when set, the grid and selection screenshots are kept there
case $server_socket in
  "$scratch"/*) ;;
  *) echo "FAIL: isolation: session socket $server_socket is outside the scratch folder $scratch"; exit 1 ;;
esac

kill_started() {
  local pid
  for pid in "${pids[@]}"; do
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  done
  pids=()
}

# stop_server: stops the session server the windows started on the scratch socket, found by its exact argument
# list (the socket path is unique to this run) and stopped by process ID.
stop_server() {
  local entry pid
  for entry in /proc/[0-9]*; do
    pid=${entry#/proc/}
    if tr '\0' ' ' <"$entry/cmdline" 2>/dev/null | grep -qF -- "--server --socket $server_socket "; then
      kill "$pid" 2>/dev/null || true
    fi
  done
}

cleanup() {
  kill_started
  stop_server
  rm -rf "${scratch:?}"
}
trap cleanup EXIT

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

# ----------------------------------------------------------------------------------------------- --faults
if [[ ${1:-} == --faults ]]; then
  caught=0
  expected_count=$((${#FAULT_PAIRS[@]} * ${#SCALES[@]} + 1))
  for scale in "${SCALES[@]}"; do
    python3 "$CHECK_DIR/expected.py" --scale "$scale" >"$scratch/expected-$scale.json"
    python3 "$CHECK_DIR/draw_reference.py" "$scratch/expected-$scale.json" "$scratch/reference.png"
    if python3 "$CHECK_DIR/check_cells.py" "$scratch/reference.png" "$scratch/expected-$scale.json" >/dev/null; then
      ok "reference image at scale $scale passes every check"
    else
      fail "reference image at scale $scale does not pass"
    fi
    for pair in "${FAULT_PAIRS[@]}"; do
      fault=${pair%%:*}
      check=${pair##*:}
      python3 "$CHECK_DIR/draw_reference.py" "$scratch/expected-$scale.json" "$scratch/fault.png" --fault "$fault"
      if output=$(python3 "$CHECK_DIR/check_cells.py" "$scratch/fault.png" "$scratch/expected-$scale.json"); then
        fail "planted fault $fault at scale $scale was not caught"
      else
        failing=$(grep '^FAIL' <<<"$output" | sed 's/^FAIL: \([a-z]*\) .*/\1/' | sort -u | tr '\n' ' ')
        if [[ $failing == "$check " ]]; then
          echo "CAUGHT: $fault at scale $scale (check $check)"
          caught=$((caught + 1))
        else
          fail "planted fault $fault at scale $scale failed the wrong checks: ${failing:-none}, expected $check"
        fi
      fi
    done
  done
  if output=$(bash "$CHECK_DIR/check_engine_free.sh" --fault 2>&1) && [[ $output == *"FAIL: engine-free clanky-core depends on gpui"* ]]; then
    echo "CAUGHT: gpui-ce in clanky-core (check engine-free)"
    caught=$((caught + 1))
  else
    fail "planted engine fault was not caught: $output"
  fi
  if ((failed == 0 && caught == expected_count)); then
    echo "PASS: gate catches all $caught planted faults"
    exit 0
  fi
  echo "FAIL: gate caught $caught of $expected_count planted faults"
  exit 1
fi

# ----------------------------------------------------------------------------------------------- step 1
if [[ -n ${CLANKY_GPUI_BIN:-} ]]; then
  binary=$CLANKY_GPUI_BIN
  ok "build clanky-gpui skipped, using CLANKY_GPUI_BIN=$binary"
else
  if [[ ! -f crates/clanky-gpui/Cargo.toml ]]; then
    echo "FAIL: build clanky-gpui crate missing"
    exit 1
  fi
  if flock "$BUILD_LOCK" cargo build --release -j 8 -p clanky-gpui; then
    ok "build clanky-gpui"
  else
    echo "FAIL: build clanky-gpui did not build"
    exit 1
  fi
  binary=$CARGO_TARGET_DIR/release/clanky-gpui
fi

# ----------------------------------------------------------------------------------------------- helpers
free_display() {
  local number=100
  while [[ -e /tmp/.X11-unix/X$number || -e /tmp/.X$number-lock ]]; do number=$((number + 1)); done
  echo "$number"
}

start_display() {
  local number waited=0
  number=$(free_display)
  Xvfb ":$number" -screen 0 "$DISPLAY_SIZE" -nolisten tcp >"$scratch/xvfb-$number.log" 2>&1 &
  pids+=($!)
  until [[ -S /tmp/.X11-unix/X$number ]]; do
    sleep 0.2
    waited=$((waited + 1))
    if ((waited > 50)); then
      echo "Xvfb :$number did not start" >&2
      return 1
    fi
  done
  echo "$number"
}

# The config's terminal colours come from the Graphite dark theme file, in the format Clanky's `terminal_colors` reads.
# The theme has no cursor or selection role: the cursor takes the foreground and the selection the accent colour.
write_config() {
  mkdir -p "$scratch/home"
  python3 - "$CHECK_DIR/graphite-dark.json" "$scratch/home/config.json" <<'PY'
import json, sys
colors = json.load(open(sys.argv[1]))
theme = colors["colors"]
config = {
    "font_family": "JetBrains Mono", "font_size": 16, "cell_height_scale": 1.2, "theme": "dark",
    "restore_layout": False, "startup_animation": False, "history": False, "animations": False, "cursor_blink": "off",
    "terminal_colors": {
        "background": theme["background"], "foreground": theme["foreground"], "cursor": theme["foreground"],
        "selection": theme["accent"], "ansi": colors["ansi"],
    },
}
json.dump(config, open(sys.argv[2], "w"))
PY
}

# launch SCALE COMMAND: starts clanky-gpui on $display_number and sets app_pid and window_id.
launch() {
  local scale=$1 command=$2 waited=0 folder
  for folder in runtime config state data cache; do mkdir -p "$scratch/xdg/$folder"; done
  chmod 700 "$scratch/xdg" "$scratch/xdg"/*
  # Without WAYLAND_DISPLAY gone, gpui-ce opens the window on the user's own Wayland desktop instead of Xvfb.
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS -u XDG_SESSION_TYPE \
    HOME=$scratch/home DISPLAY=:$display_number GPUI_X11_SCALE_FACTOR=$scale \
    XDG_RUNTIME_DIR=$scratch/xdg/runtime XDG_CONFIG_HOME=$scratch/xdg/config XDG_STATE_HOME=$scratch/xdg/state \
    XDG_DATA_HOME=$scratch/xdg/data XDG_CACHE_HOME=$scratch/xdg/cache \
    "$binary" --config "$scratch/home/config.json" --size "$WINDOW_SIZE" --exec "$command" --socket "$server_socket" \
    >"$scratch/app.log" 2>&1 &
  app_pid=$!
  pids+=("$app_pid")
  window_id=
  while [[ -z $window_id ]]; do
    window_id=$(DISPLAY=:$display_number xdotool search --name '^clanky-gpui$' 2>/dev/null | head -n 1 || true)
    sleep 0.25
    waited=$((waited + 1))
    if ((waited > WAIT_SECONDS * 4)); then
      echo "note: the window's last output: $(tail -n 3 "$scratch/app.log" | tr '\n' ' ')"
      return 1
    fi
  done
}

stop_app() {
  kill "$app_pid" 2>/dev/null || true
  wait "$app_pid" 2>/dev/null || true
}

# row_10_has_ink PNG EXPECTED: true when the full block in row 10 shows in the screenshot.
row_10_has_ink() {
  python3 - "$1" "$2" "$CHECK_DIR" <<'PY'
import json, sys
from PIL import Image
image = Image.open(sys.argv[1]).convert("RGB")
cell = next(c for c in json.load(open(sys.argv[2]))["cells"] if c["row"] == 10 and c["col"] == 0)
x, y = int(cell["left"] + cell["width"] / 2), int(cell["top"] + cell["height"] / 2)
theme = json.load(open(sys.argv[3] + "/graphite-dark.json"))["colors"]["background"]
background = tuple(int(theme[i:i + 2], 16) for i in (1, 3, 5))
sys.exit(0 if x < image.width and y < image.height and image.getpixel((x, y)) != background else 1)
PY
}

# screenshot SCALE OUT: waits for row 10 to show (at most WAIT_SECONDS), then keeps the last screenshot.
screenshot() {
  local out=$2 waited=0
  while true; do
    DISPLAY=:$display_number import -window "$window_id" "$out" 2>/dev/null || true
    if [[ -s $out ]] && row_10_has_ink "$out" "$scratch/expected-$1.json"; then
      return 0
    fi
    sleep 0.5
    waited=$((waited + 1))
    if ((waited > WAIT_SECONDS * 2)); then
      return 1
    fi
  done
}

# run_checks LABEL PNG SCALE [EXTRA ARGUMENTS...]: prints check_cells.py's lines; a failing line fails the step.
run_checks() {
  local label=$1 png=$2 scale=$3
  shift 3
  if output=$(python3 "$CHECK_DIR/check_cells.py" "$png" "$scratch/expected-$scale.json" "$@"); then
    ok "$label (scale $scale): $(tr '\n' ';' <<<"${output//ok: /}")"
  else
    fail "$label (scale $scale): $(grep '^FAIL' <<<"$output" | head -n 3 | tr '\n' ';')"
  fi
}

selection_colour() {
  python3 - "$scratch/home/config.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["terminal_colors"]["selection"])
PY
}

# ----------------------------------------------------------------------------------------------- steps 2 to 4
screen_checks() {
  local scale=$1 shape sx sy ex selection_colour
  python3 "$CHECK_DIR/expected.py" --scale "$scale" >"$scratch/expected-$scale.json"
  rm -rf "${scratch:?}/home"
  write_config
  display_number=$(start_display) || { fail "display for scale $scale"; return; }

  if ! launch "$scale" "$FIXTURE_COMMAND"; then
    fail "window clanky-gpui did not appear within ${WAIT_SECONDS}s at scale $scale"
    return
  fi
  if screenshot "$scale" "$scratch/grid-$scale.png"; then
    run_checks "grid fixture" "$scratch/grid-$scale.png" "$scale"
    [[ -z $SHOTS_DIR ]] || { mkdir -p "$SHOTS_DIR"; cp "$scratch/grid-$scale.png" "$SHOTS_DIR/clanky-gpui-grid-scale-$scale.png"; }
  else
    fail "row 10 did not show within ${WAIT_SECONDS}s at scale $scale"
  fi

  # Selection: drag from the left quarter of row 0 column 10 to the right quarter of column 30 (device pixels).
  selection_colour=$(selection_colour)
  echo "note: theme has no selection role, using accent $selection_colour as terminal_colors.selection"
  read -r sx sy ex < <(python3 - "$scratch/expected-$scale.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
first = next(c for c in data["cells"] if c["row"] == 0 and c["col"] == 10)
last = next(c for c in data["cells"] if c["row"] == 0 and c["col"] == 30)
print(int(first["left"] + first["width"] * 0.25), int(first["top"] + first["height"] / 2), int(last["left"] + last["width"] * 0.75))
PY
  )
  DISPLAY=:$display_number xdotool mousemove --window "$window_id" "$sx" "$sy" mousedown 1
  DISPLAY=:$display_number xdotool mousemove --window "$window_id" "$(((sx + ex) / 2))" "$sy"
  DISPLAY=:$display_number xdotool mousemove --window "$window_id" "$ex" "$sy" mouseup 1
  sleep 0.5
  DISPLAY=:$display_number import -window "$window_id" "$scratch/selection-$scale.png" 2>/dev/null || true
  run_checks "selection" "$scratch/selection-$scale.png" "$scale" --selection "0,10,30,$selection_colour"
  [[ -z $SHOTS_DIR ]] || { mkdir -p "$SHOTS_DIR"; cp "$scratch/selection-$scale.png" "$SHOTS_DIR/clanky-gpui-selection-scale-$scale.png"; }
  stop_app

  # Cursor: one window per shape; the fixture leaves the cursor at row 11, column 0.
  for shape in block:'\e[2 q' bar:'\e[6 q' underline:'\e[4 q'; do
    if ! launch "$scale" "$FIXTURE_COMMAND; printf '${shape#*:}'"; then
      fail "window for the ${shape%%:*} cursor at scale $scale"
      continue
    fi
    if screenshot "$scale" "$scratch/cursor-$scale.png"; then
      run_checks "cursor ${shape%%:*}" "$scratch/cursor-$scale.png" "$scale" --cursor "11,0,${shape%%:*}"
    else
      fail "row 10 did not show for the ${shape%%:*} cursor at scale $scale"
    fi
    stop_app
  done
}

(
  flock 9
  trap kill_started EXIT   # a subshell does not keep the parent's trap: stop its Xvfb and window here
  for scale in "${SCALES[@]}"; do
    screen_checks "$scale"
  done
  # The subshell cannot set the parent's flag: tell the parent through a file.
  echo "$failed" >"$scratch/screen-failed"
) 9>"$GATE_LOCK"
if [[ $(cat "$scratch/screen-failed" 2>/dev/null || echo 1) != 0 ]]; then failed=1; fi

# ----------------------------------------------------------------------------------------------- step 5
# Today's clanky and clanky-gpui alternate, SPEED_RUNS runs each, so drift on the machine hits both; medians are compared.
if [[ -z $today_binary ]]; then
  if [[ -d $TODAY_WORKTREE ]] && flock "$BUILD_LOCK" env CARGO_TARGET_DIR="$TODAY_WORKTREE/target" \
    cargo build -q --release --offline --locked -j 8 --manifest-path "$TODAY_WORKTREE/Cargo.toml" -p clanky --bin clanky; then
    today_binary=$TODAY_WORKTREE/target/release/clanky
  fi
fi
if [[ -f $CHECK_DIR/speed.sh && -x ${today_binary:-/nonexistent} ]]; then
  speed_failed=0
  for run in $(seq "$SPEED_RUNS"); do
    for name in today gpui; do
      if [[ $name == today ]]; then run_binary=$today_binary; else run_binary=$binary; fi
      bash "$CHECK_DIR/speed.sh" "$run_binary" --runs 1 --out "$scratch/speed-$name-$run.json" >"$scratch/speed.log" 2>&1 ||
        { fail "speed harness failed for $name run $run: $(tail -n 1 "$scratch/speed.log")"; speed_failed=1; }
    done
  done
  if ((speed_failed == 0)); then
    if verdict=$(python3 - "$scratch" "$SPEED_RUNS" "$SPEED_LIMIT_TIME" "$SPEED_LIMIT_KEY_MS" "$SPEED_LIMIT_START" <<'PY'
import json, statistics, sys
folder, runs = sys.argv[1], int(sys.argv[2])
time_limit, key_limit, start_limit = (float(value) for value in sys.argv[3:6])


def median(name, key):
    values = [json.load(open(f"{folder}/speed-{name}-{run}.json"))["summary"][key]["median"] for run in range(1, runs + 1)]
    return statistics.median(values)


limits = {
    "cat_s": lambda n, t: n <= time_limit * t,
    "key_p50_ms": lambda n, t: n <= t + key_limit,
    "key_p99_ms": lambda n, t: n <= t + key_limit,
    "idle_draws": lambda n, t: n == 0,
    "start_ms": lambda n, t: n <= start_limit * t,
    "memory_mib": lambda n, t: n <= t,
}
rows = {key: (median("gpui", key), median("today", key)) for key in limits}
bad = [f"{key} {now:.2f} against today's {then:.2f}" for key, (now, then) in rows.items() if not limits[key](now, then)]
print("; ".join(bad) if bad else "six numbers within the limits (medians of " + str(runs) + " alternating runs)")
sys.exit(1 if bad else 0)
PY
    ); then
      ok "speed: $verdict"
    else
      fail "speed: $verdict"
    fi
  fi
else
  fail "speed harness or today's clanky missing"
fi

# ----------------------------------------------------------------------------------------------- step 6
for name in gate-t2-terminal gate-mouse gate-exit gate-c2-schemes gate-m12-idle check-robot-startup check-robot-window; do
  files=(scripts/"$name".*)
  if ! grep -q CLANKY_BIN "${files[@]}" 2>/dev/null; then
    fail "$name not retargeted"
    continue
  fi
  if [[ -f scripts/$name.sh ]]; then command=(bash "scripts/$name.sh"); else command=(python3 "scripts/$name.py"); fi
  if output=$(CLANKY_BIN=$binary flock "$GATE_LOCK" "${command[@]}" 2>&1); then
    ok "$name: $(tail -n 1 <<<"$output")"
  else
    fail "$name: $(tail -n 1 <<<"$output")"
  fi
done

# ----------------------------------------------------------------------------------------------- step 7
if output=$(bash "$CHECK_DIR/check_engine_free.sh" 2>&1); then ok "$output"; else fail "$output"; fi

if ((failed)); then
  echo "FAIL: terminal pane"
  exit 1
fi
echo "PASS: terminal pane"
