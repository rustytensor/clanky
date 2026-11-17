#!/usr/bin/env bash
# Starts clanky-gpui on the X display in DISPLAY (no Wayland) and prints the xprop answer for its window's WM_CLASS.
# Usage: check_x11_class.sh BINARY RUNTIME_DIR HOME_DIR XDG_PARENT CONFIG_FILE
# Prints xprop's line, or "WM_CLASS: no window within SECONDS s" and exits 1 when the window never shows. The caller
# stops the session server the window started.
set -euo pipefail
binary=$1 runtime=$2 home=$3 xdg=$4 config=$5
WINDOW_SECONDS=10

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_SESSION_TYPE=x11 \
  HOME="$home" XDG_RUNTIME_DIR="$runtime" XDG_CONFIG_HOME="$xdg/config" XDG_STATE_HOME="$xdg/state" \
  XDG_DATA_HOME="$xdg/data" XDG_CACHE_HOME="$xdg/cache" VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
  "$binary" --config "$config" >/dev/null 2>&1 &
app=$!
trap 'kill "$app" 2>/dev/null || true; wait "$app" 2>/dev/null || true' EXIT

for ((tick = 0; tick < WINDOW_SECONDS * 10; tick++)); do
  window=$(xdotool search --onlyvisible --pid "$app" 2>/dev/null | head -n 1 || true)
  if [[ -n $window ]]; then
    xprop -id "$window" WM_CLASS
    exit 0
  fi
  sleep 0.1
done
echo "WM_CLASS: no window within ${WINDOW_SECONDS} s"
exit 1
