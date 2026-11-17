#!/usr/bin/env bash
# Terminal pane gate, checks 9 and 10: clanky-gpui started the way a user starts it, on Wayland.
# Check 9: under a headless sway (own runtime folder, XWayland off, never the user's session), the default config
#   with no --exec opens a native Wayland window within WINDOW_SECONDS, the prompt row has ink before typing, and after
#   typing `echo g1-ok` and Enter the text g1-ok shows twice (the typed line and its output).
# Check 10: the same start with a runtime folder that has the setgid bit either opens a window that takes the same
#   typing, or exits 1 with one line that names the folder and the fix.
# Prints "ok: ..." or "FAIL: ..." per check, then PASS or FAIL; exit 1 on any failure.
# --faults needs no binary and no sway: it shows the screenshot reader accepting a window drawn as it should look and
#   rejecting one whose prompt row shows only the cursor (the planted fault: command_input "editor" honoured, which shows a bare cursor).
# CLANKY_GPUI_BIN=PATH names the binary (default target/release/clanky-gpui under CARGO_TARGET_DIR).
# Tools: sway, swaymsg, wtype (types through the virtual keyboard protocol), grim (screenshots), python3 with Pillow.
set -euo pipefail
cd "$(dirname "$0")/../.."

CHECK_DIR=scripts/terminal-pane
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}
binary=${CLANKY_GPUI_BIN:-$CARGO_TARGET_DIR/release/clanky-gpui}
WINDOW_SECONDS=2      # check 9: the window must be mapped this soon after the start
SETTLE_SECONDS=10     # check 10 and the first frame: the longest we wait for a window or its ink
TYPE_SECONDS=5        # the typed text must show within this long
SETGID_FOLDER_MODE=2700
RUNTIME_PARENT=/run/user/$(id -u)

scratch=$(mktemp -d /srv/devops/build/wayland-check.XXXXXX)
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
    fail "planted fault (command_input editor honoured, bare cursor) was not caught"
  else
    echo "CAUGHT: command_input editor honoured (check 9 prompt row): $output"
  fi
  if ((failed)); then
    echo "FAIL: wayland checks catch 0 of 1 planted faults"
    exit 1
  fi
  echo "PASS: wayland checks catch all 1 planted faults"
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
mkdir -p "$scratch/home"
for folder in config state data cache; do private_folder "$scratch/xdg/$folder" 700; done
private_folder "$scratch/home" 700

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

# launch RUNTIME SOCKET_NAME: starts clanky-gpui as a user would (default config file, no --exec, no --socket) with
# XDG_RUNTIME_DIR=RUNTIME and WAYLAND_DISPLAY=SOCKET_NAME; sets app_pid and app_start (seconds).
launch() {
  mkdir -p "$scratch/app-config"
  echo '{}' >"$scratch/app-config/config.json"
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

type_echo() {
  on_sway wtype -s 500 'echo g1-ok'
  on_sway wtype -k Return
}

# keep_shot NAME: with CLANKY_WAYLAND_SHOTS set, keeps the last screenshot there as NAME.png.
keep_shot() {
  [[ -z ${CLANKY_WAYLAND_SHOTS:-} || ! -f $scratch/screen.png ]] || { mkdir -p "$CLANKY_WAYLAND_SHOTS"; cp "$scratch/screen.png" "$CLANKY_WAYLAND_SHOTS/$1.png"; }
}

stop_app() {
  kill "$app_pid" 2>/dev/null || true
  wait "$app_pid" 2>/dev/null || true
}

# ----------------------------------------------------------------------------------------------- check 9
check_9() {
  local line
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
  if line=$(wait_screen "$SETTLE_SECONDS" prompt); then ok "${line#ok: }"; else fail "${line#FAIL: }"; fi
  keep_shot "check-9-start"
  type_echo
  if line=$(wait_screen "$TYPE_SECONDS" text 2 --exactly); then ok "${line#ok: }"; else fail "${line#FAIL: }"; fi
  keep_shot "check-9-typed"
  stop_app
  stop_server "$sway_runtime"
}

# ----------------------------------------------------------------------------------------------- check 10
check_10() {
  local line status message
  setgid_runtime=$(mktemp -d "$RUNTIME_PARENT/clanky-setgid-check.XXXXXX")
  private_folder "$setgid_runtime" "$SETGID_FOLDER_MODE"
  launch "$setgid_runtime" "$wayland_socket"
  if wait_window "$SETTLE_SECONDS"; then
    type_echo
    if line=$(wait_screen "$TYPE_SECONDS" text 1); then
      ok "wayland (check 10): setgid runtime folder, window opened and took typing: ${line#ok: }"
    else
      fail "wayland (check 10): setgid runtime folder, window opened but typing did not show: ${line#FAIL: }"
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
check_10
if ((failed)); then
  echo "FAIL: wayland start"
  exit 1
fi
echo "PASS: wayland start"
