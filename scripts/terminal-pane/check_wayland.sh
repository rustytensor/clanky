#!/usr/bin/env bash
# Terminal pane gate, checks 9 and 10: clanky-gpui started the way a user starts it, on Wayland.
# Check 9: under a headless sway (own runtime folder, XWayland off, never the user's session), the default config
#   with no --exec opens a native Wayland window within WINDOW_SECONDS, the prompt row has ink before typing, and after
#   typing `echo g1-ok` and Enter the text g1-ok shows twice (the typed line and its output).
# Check 9, defaults: the same with the config a user has (the command input and two panes): ink in the input line before
#   typing, the typed line in it, and after Enter g1-ok twice in the focused pane (its block's command row and the
#   output). Check 9 above is the classic one-pane run.
# Check 9 also reads the window's app_id from sway's tree: "clanky". Check 11: the same window on a private Xvfb, no
#   Wayland: xprop's WM_CLASS is "clanky", "clanky".
# Check 10: the same start with a runtime folder that has the setgid bit either opens a window that takes the same
#   typing (g1-ok twice, as in check 9), or exits 1 with one line that names the folder and the fix.
# Prints "ok: ..." or "FAIL: ..." per check, then PASS or FAIL; exit 1 on any failure.
# Every wait ends when its condition holds, with a deadline of 30 s for a busy machine; keys are sent once, after the window
#   has drawn its prompt row or command input, and a failure to show them reports when they were sent.
# --faults needs no binary and no sway: it shows the screenshot reader accepting a window drawn as it should look and
#   rejecting one whose prompt row shows only the cursor (a prompt row with no prompt drawn, as the editor input would
#   show), and the default window's reader accepting each stage and rejecting typed text that never reached the shell.
# CLANKY_GPUI_BIN=PATH names the binary (default target/release/clanky-gpui under CARGO_TARGET_DIR).
# Tools: sway, swaymsg, wtype (types through the virtual keyboard protocol), grim (screenshots), python3 with Pillow.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=../private-display.sh
source scripts/private-display.sh

CHECK_DIR=scripts/terminal-pane
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}
binary=${CLANKY_GPUI_BIN:-$CARGO_TARGET_DIR/release/clanky-gpui}
WINDOW_SECONDS=2      # check 9: the window must be mapped this soon after the start
# The deadlines are long because a busy machine can take many seconds to draw the window's layout; each wait ends as
# soon as its condition holds. Typing starts only after the window has shown that it is ready (the prompt row's ink, or
# the command input's), and is sent once: text lost after that point is a fault of the window, and the failure says
# how long after the start the keys were sent.
SETTLE_SECONDS=30     # the longest we wait for a window, its first ink or its command bar
TYPE_SECONDS=30       # the typed text must show within this long
STAGE_MS=6000         # the default window: the typed line waits this long in the input before Return
SETGID_FOLDER_MODE=2700
RUNTIME_PARENT=/run/user/$(id -u)

scratch=$(mktemp -d "${CLANKY_GATE_TMP:-${TMPDIR:-/tmp}}/wayland-check.XXXXXX")
unset WAYLAND_DISPLAY DISPLAY DBUS_SESSION_BUS_ADDRESS XDG_SESSION_TYPE SWAYSOCK I3SOCK
pids=()
failed=0
sway_runtime=
setgid_runtime=

ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }

kill_started() {
  local pid
  for pid in "${pids[@]}"; do
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  done
  pids=()
}

# stop_server RUNTIME: stops the session server the window started, found by its exact argument list and stopped by
# process ID.
stop_server() {
  local entry pid
  for entry in /proc/[0-9]*; do
    pid=${entry#/proc/}
    # A process may end while the list is read; its missing cmdline is not an error.
    if { tr '\0' ' ' <"$entry/cmdline"; } 2>/dev/null | grep -qF -- "--server --socket $1/clanky/server.sock "; then
      kill "$pid" 2>/dev/null || true
    fi
  done
}

cleanup() {
  kill_started
  [[ -z $sway_runtime ]] || stop_server "$sway_runtime"
  [[ -z $setgid_runtime ]] || stop_server "$setgid_runtime"
  rm -rf -- "${scratch:?}"
  [[ -z $sway_runtime ]] || rm -rf -- "${sway_runtime:?}"
  [[ -z $setgid_runtime ]] || rm -rf -- "${setgid_runtime:?}"
}
trap cleanup EXIT

# ----------------------------------------------------------------------------------------------- --faults
if [[ ${1:-} == --faults ]]; then
  python3 "$CHECK_DIR/check_wayland.py" draw "$scratch/good.png"
  python3 "$CHECK_DIR/check_wayland.py" draw "$scratch/fault.png" --fault cursor-only
  if python3 "$CHECK_DIR/check_wayland.py" prompt "$scratch/good.png" >/dev/null \
    && python3 "$CHECK_DIR/check_wayland.py" text "$scratch/good.png" 2 --exactly >/dev/null; then
    ok "reader accepts a window with a prompt and g1-ok twice"
  else
    fail "reader rejects a window drawn as it should look"
  fi
  if output=$(python3 "$CHECK_DIR/check_wayland.py" prompt "$scratch/fault.png"); then
    fail "planted fault (a prompt row with only the cursor) was not caught"
  else
    echo "CAUGHT: a prompt row with only the cursor (check 9 prompt row): $output"
  fi
  for stage in input typed output; do
    python3 "$CHECK_DIR/check_wayland.py" draw-default "$scratch/default-$stage.png" --stage "$stage"
  done
  python3 "$CHECK_DIR/check_wayland.py" draw-default "$scratch/default-fault.png" --fault typed-not-sent
  if python3 "$CHECK_DIR/check_wayland.py" input "$scratch/default-input.png" >/dev/null \
    && python3 "$CHECK_DIR/check_wayland.py" typed "$scratch/default-typed.png" >/dev/null \
    && python3 "$CHECK_DIR/check_wayland.py" output "$scratch/default-output.png" >/dev/null; then
    ok "reader accepts the default window at each stage: ink in the input, the typed line in it, g1-ok twice after Enter"
  else
    fail "reader rejects the default window drawn as it should look"
  fi
  if output=$(python3 "$CHECK_DIR/check_wayland.py" output "$scratch/default-fault.png"); then
    fail "planted fault (typed text that never reaches the shell) was not caught"
  else
    echo "CAUGHT: typed text never reaching the shell (the default window's output check): $output"
  fi
  python3 "$CHECK_DIR/check_wayland.py" draw-tree "$scratch/tree-good.json"
  python3 "$CHECK_DIR/check_wayland.py" draw-tree "$scratch/tree-fault.json" --fault app-id-unset
  python3 "$CHECK_DIR/check_wayland.py" draw-xprop "$scratch/xprop-good.txt"
  python3 "$CHECK_DIR/check_wayland.py" draw-xprop "$scratch/xprop-fault.txt" --fault app-id-unset
  if python3 "$CHECK_DIR/check_wayland.py" app-id <"$scratch/tree-good.json" >/dev/null \
    && python3 "$CHECK_DIR/check_wayland.py" wm-class <"$scratch/xprop-good.txt" >/dev/null; then
    ok "reader accepts a window whose app id is clanky on Wayland and on X11"
  else
    fail "reader rejects a window with app id clanky"
  fi
  for platform in "app-id tree" "wm-class xprop"; do
    read -r reader file <<<"$platform"
    input="$scratch/$file-fault"
    [[ $file == tree ]] && input+=.json || input+=.txt
    if output=$(python3 "$CHECK_DIR/check_wayland.py" "$reader" <"$input"); then
      fail "planted fault (app id unset) was not caught by the $reader reader"
    else
      echo "CAUGHT: app id unset ($reader): $output"
    fi
  done
  if ((failed)); then
    echo "FAIL: wayland checks catch 0 of 4 planted faults"
    exit 1
  fi
  echo "PASS: wayland checks catch all 4 planted faults"
  exit 0
fi

# ----------------------------------------------------------------------------------------------- headless sway
for tool in sway swaymsg wtype grim; do
  command -v "$tool" >/dev/null || { echo "FAIL: wayland needs $tool, which is not installed"; exit 1; }
done
[[ -x $binary ]] || { echo "FAIL: wayland binary $binary not found (set CLANKY_GPUI_BIN)"; exit 1; }

# private_folder PATH MODE: makes PATH and checks that its mode is exactly MODE (the setgid bit is inherited on some
# disks, and the session server wants a runtime folder of exactly 0700).
private_folder() {
  mkdir -p "$1"
  if [[ $2 == 700 ]]; then chmod g-s,u=rwx,go= "$1"; else chmod "$2" "$1"; fi
  [[ $(stat -c %a "$1") == "$2" ]] || { echo "FAIL: isolation: $1 has mode $(stat -c %a "$1"), not $2"; exit 1; }
}

sway_runtime=$(mktemp -d "$RUNTIME_PARENT/clanky-wayland-check.XXXXXX")
private_folder "$sway_runtime" 700
case $sway_runtime in
  "$RUNTIME_PARENT"/clanky-wayland-check.*) ;;
  *) echo "FAIL: isolation: sway runtime folder $sway_runtime is outside $RUNTIME_PARENT"; exit 1 ;;
esac
# fresh_state: empty home and XDG folders, so that no start restores the layout or the history of the start before it.
fresh_state() {
  rm -rf -- "${scratch:?}/xdg" "${scratch:?}/home"
  mkdir -p "$scratch/home"
  for folder in config state data cache; do private_folder "$scratch/xdg/$folder" 700; done
  private_folder "$scratch/home" 700
}
fresh_state

cat >"$scratch/sway.conf" <<'EOF'
xwayland disable
default_border none
default_floating_border none
EOF
env -u WAYLAND_DISPLAY -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
  XDG_RUNTIME_DIR="$sway_runtime" HOME="$scratch/home" WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 \
  WLR_RENDERER=pixman sway --config "$scratch/sway.conf" >"$scratch/sway.log" 2>&1 &
pids+=($!)
waited=0
until compgen -G "$sway_runtime/sway-ipc.*.sock" >/dev/null && compgen -G "$sway_runtime/wayland-*" >/dev/null; do
  sleep 0.2
  waited=$((waited + 1))
  if ((waited > 50)); then
    echo "FAIL: wayland headless sway did not start: $(tail -n 2 "$scratch/sway.log" | tr '\n' ' ')"
    exit 1
  fi
done
SWAYSOCK=$(compgen -G "$sway_runtime/sway-ipc.*.sock" | head -n 1)
wayland_socket=$(compgen -G "$sway_runtime/wayland-*" | grep -v '\.lock$' | head -n 1)
export SWAYSOCK

# Every client of the headless sway: only its own runtime folder and socket, never the user's session.
on_sway() {
  env -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$sway_runtime" WAYLAND_DISPLAY="$(basename "$wayland_socket")" "$@"
}

classic_config='{"font_family": "JetBrains Mono", "font_size": 16, "cell_height_scale": 1.2, "tools_panel": "off", "new_window_panes": 1, "command_input": "classic"}'
# The window as a user gets it, but for the terminal font and size (the reader's cell geometry depends on them; the
# default font is a moving target): the command input, two panes and the tools panel are the defaults.
default_config='{"font_family": "JetBrains Mono", "font_size": 16, "cell_height_scale": 1.2}'
# launch RUNTIME SOCKET_NAME [CONFIG_JSON]: starts clanky-gpui as a user would (default config file, no --exec, no --socket) with
# XDG_RUNTIME_DIR=RUNTIME and WAYLAND_DISPLAY=SOCKET_NAME; sets app_pid and app_start (seconds).
launch() {
  fresh_state
  mkdir -p "$scratch/app-config"
  # The default config but for the terminal font and size, which the screenshot reader (expected.py's cell geometry,
  # JetBrains Mono at 16 px with 1.2 line height) depends on; the default font is a moving target. The tools panel is
  # off and the window opens one pane because that geometry is one pane with no panel beside it. The input is the
  # classic one, where the shell draws its prompt row: these checks prove the native Wayland window and its typing,
  # and the command input that is the default since G2 has its own gate (q2-input).
  echo "${3:-$classic_config}" >"$scratch/app-config/config.json"
  env -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS -u XDG_SESSION_TYPE \
    HOME="$scratch/home" WAYLAND_DISPLAY="$2" XDG_RUNTIME_DIR="$1" XDG_CONFIG_HOME="$scratch/xdg/config" \
    XDG_STATE_HOME="$scratch/xdg/state" XDG_DATA_HOME="$scratch/xdg/data" XDG_CACHE_HOME="$scratch/xdg/cache" \
    VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
    "$binary" --config "$scratch/app-config/config.json" >"$scratch/app.out" 2>"$scratch/app.err" &
  app_pid=$!
  pids+=("$app_pid")
  app_start=$(date +%s.%N)
}

# wait_window SECONDS: sets window_geometry ("X Y W H SHELL") when sway shows a window, in at most SECONDS; returns 1
# when none shows or the app has exited.
wait_window() {
  local limit=$1 now
  while true; do
    if window_geometry=$(swaymsg -t get_tree 2>/dev/null | python3 "$CHECK_DIR/check_wayland.py" window); then
      window_seconds=$(python3 -c "import time,sys; print(f'{time.time()-float(sys.argv[1]):.2f}')" "$app_start")
      return 0
    fi
    kill -0 "$app_pid" 2>/dev/null || return 1
    now=$(date +%s.%N)
    python3 -c "import sys; sys.exit(0 if float(sys.argv[1]) - float(sys.argv[2]) < float(sys.argv[3]) else 1)" "$now" "$app_start" "$limit" || return 1
    sleep 0.1
  done
}

# shoot OUT: screenshot of the window's own rectangle.
shoot() {
  local x y width height
  read -r x y width height _ <<<"$window_geometry"
  on_sway grim -g "$x,$y ${width}x${height}" "$1"
}

# wait_screen SECONDS READER_ARGS...: takes screenshots until check_wayland.py's reader accepts one (at most SECONDS);
# prints the reader's last line.
wait_screen() {
  local limit=$1 waited=0 output
  shift
  while true; do
    shoot "$scratch/screen.png"
    if output=$(python3 "$CHECK_DIR/check_wayland.py" "$1" "$scratch/screen.png" "${@:2}"); then
      echo "$output"
      return 0
    fi
    waited=$((waited + 1))
    if ((waited * 5 >= limit * 10)); then
      echo "$output"
      return 1
    fi
    sleep 0.5
  done
}

# type_echo_staged: one wtype process types the line, waits, and presses Return, so that the typed line can be seen in
# the input before it is run; it runs in the background and typer_pid is its process ID.
type_echo_staged() {
  on_sway wtype -s 500 'echo g1-ok' -s "$STAGE_MS" -k Return &
  typer_pid=$!
  pids+=("$typer_pid")
}

# One wtype process types the line and presses Return: each process makes its own virtual keyboard, and the first key
# a new keyboard sends is lost, so a second process for Return left the typed line unrun in 3 of 10 runs.
type_echo() {
  on_sway wtype -s 500 'echo g1-ok' -k Return
}

# keep_shot NAME: with CLANKY_WAYLAND_SHOTS set, keeps the last screenshot there as NAME.png.
keep_shot() {
  [[ -z ${CLANKY_WAYLAND_SHOTS:-} || ! -f $scratch/screen.png ]] || { mkdir -p "$CLANKY_WAYLAND_SHOTS"; cp "$scratch/screen.png" "$CLANKY_WAYLAND_SHOTS/$1.png"; }
}

# since_start: seconds since the app was started, for the failures that report when the keys were sent.
since_start() {
  python3 -c "import time,sys; print(f'{time.time()-float(sys.argv[1]):.1f}')" "$app_start"
}

stop_app() {
  kill "$app_pid" 2>/dev/null || true
  wait "$app_pid" 2>/dev/null || true
}

# ----------------------------------------------------------------------------------------------- check 9
check_9() {
  local line keys_at
  launch "$sway_runtime" "$(basename "$wayland_socket")"
  if ! wait_window "$SETTLE_SECONDS"; then
    fail "wayland (check 9): no window within ${SETTLE_SECONDS}s; the app said: $(tail -n 2 "$scratch/app.err" | tr '\n' ' ')"
    stop_app
    return
  fi
  if [[ ${window_geometry##* } != xdg_shell ]]; then
    fail "wayland (check 9): the window's shell is ${window_geometry##* }, not a native Wayland window (xdg_shell)"
  elif python3 -c "import sys; sys.exit(0 if float(sys.argv[1]) <= float(sys.argv[2]) else 1)" "$window_seconds" "$WINDOW_SECONDS"; then
    ok "wayland (check 9): native Wayland window after ${window_seconds}s"
  else
    fail "wayland (check 9): window opened after ${window_seconds}s, at most ${WINDOW_SECONDS}s expected"
  fi
  if line=$(swaymsg -t get_tree | python3 "$CHECK_DIR/check_wayland.py" app-id); then ok "${line#ok: }"; else fail "${line#FAIL: }"; fi
  if line=$(wait_screen "$SETTLE_SECONDS" prompt); then ok "${line#ok: }"; else fail "${line#FAIL: }"; fi
  keep_shot "check-9-start"
  keys_at=$(since_start)
  type_echo
  if line=$(wait_screen "$TYPE_SECONDS" text 2 --exactly); then ok "${line#ok: }"; else fail "${line#FAIL: } (keys sent ${keys_at}s after the start, window ready)"; fi
  keep_shot "check-9-typed"
  stop_app
  stop_server "$sway_runtime"
}

# The same start with the defaults: the command input and two panes, as a user gets them (the check Marc's own first
# run needs). The input line has ink before typing; the typed line shows in the input line; after Enter g1-ok shows
# twice in the focused pane, the lower one: the command in its block's command row and the output. The expected count
# is read off the block layout in a screenshot of clanky-gpui: a block is a meta row, the command row, then the output.
check_9_default() {
  local line keys_at
  launch "$sway_runtime" "$(basename "$wayland_socket")" "$default_config"
  if ! wait_window "$SETTLE_SECONDS"; then
    fail "wayland (check 9, defaults): no window within ${SETTLE_SECONDS}s; the app said: $(tail -n 2 "$scratch/app.err" | tr '\n' ' ')"
    stop_app
    return
  fi
  if line=$(wait_screen "$SETTLE_SECONDS" input); then ok "${line#ok: } (defaults)"; else fail "${line#FAIL: } (defaults)"; fi
  keep_shot "check-9-default-start"
  keys_at=$(since_start)
  type_echo_staged
  if line=$(wait_screen "$TYPE_SECONDS" typed); then ok "${line#ok: } (defaults)"; else fail "${line#FAIL: } (defaults; keys sent ${keys_at}s after the start, window ready)"; fi
  keep_shot "check-9-default-typed"
  if line=$(wait_screen "$((STAGE_MS / 1000 + TYPE_SECONDS))" output); then ok "${line#ok: } (defaults)"; else fail "${line#FAIL: } (defaults)"; fi
  keep_shot "check-9-default-output"
  wait "$typer_pid" 2>/dev/null || true
  stop_app
  stop_server "$sway_runtime"
}

# ----------------------------------------------------------------------------------------------- check 11
# The window's class on X11, as a panel or a window rule on an X11 session reads it: a private Xvfb, no Wayland, and
# xprop's WM_CLASS for the window is "clanky", "clanky".
check_11() {
  local output status=0 line
  fresh_state
  mkdir -p "$scratch/app-config"
  echo "$default_config" >"$scratch/app-config/config.json"
  output=$(run_on_private_display 1280x720x24 bash "$CHECK_DIR/check_x11_class.sh" "$binary" "$sway_runtime" \
    "$scratch/home" "$scratch/xdg" "$scratch/app-config/config.json") || status=$?
  stop_server "$sway_runtime"
  if line=$(python3 "$CHECK_DIR/check_wayland.py" wm-class <<<"$output"); then
    ok "${line#ok: } (status $status)"
  else
    fail "${line#FAIL: } (status $status)"
  fi
}

# ----------------------------------------------------------------------------------------------- check 10
check_10() {
  local line status message keys_at
  setgid_runtime=$(mktemp -d "$RUNTIME_PARENT/clanky-setgid-check.XXXXXX")
  private_folder "$setgid_runtime" "$SETGID_FOLDER_MODE"
  launch "$setgid_runtime" "$wayland_socket"
  if wait_window "$SETTLE_SECONDS"; then
    if ! wait_screen "$SETTLE_SECONDS" prompt >/dev/null; then
      fail "wayland (check 10): setgid runtime folder, window opened but its prompt row stayed empty for ${SETTLE_SECONDS}s"
      stop_app
      stop_server "$setgid_runtime"
      return
    fi
    keys_at=$(since_start)
    type_echo
    if line=$(wait_screen "$TYPE_SECONDS" text 2 --exactly); then
      ok "wayland (check 10): setgid runtime folder, window opened and took typing: ${line#ok: }"
    else
      fail "wayland (check 10): setgid runtime folder, window opened but typing did not show: ${line#FAIL: } (keys sent ${keys_at}s after the start, window ready)"
    fi
    keep_shot "check-10-typed"
    stop_app
    stop_server "$setgid_runtime"
    return
  fi
  status=0
  wait "$app_pid" 2>/dev/null || status=$?
  message=$(cat "$scratch/app.err")
  if ((status == 1)) && [[ $(wc -l <<<"$message") -eq 1 && $message == *"$setgid_runtime"* && $message =~ (chmod|g-s|setgid|0700|700) ]]; then
    ok "wayland (check 10): setgid runtime folder, exit 1 with one line: $message"
  else
    fail "wayland (check 10): setgid runtime folder, no window and exit status $status, stderr: ${message:-nothing}"
  fi
}

check_9
check_9_default
check_11
check_10
if ((failed)); then
  echo "FAIL: wayland start"
  exit 1
fi
echo "PASS: wayland start"
