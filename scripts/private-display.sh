#!/usr/bin/env bash
# Sourced by the gate wrappers: run_on_private_display GEOMETRY COMMAND [ARGS...] runs COMMAND with DISPLAY set to a
# private Xvfb, then stops that Xvfb and waits for it to be gone. Returns COMMAND's status.
#
# Why not `xvfb-run -a`: it picks a free display number from the lock files, starts Xvfb and kills it afterwards
# without waiting, so the next start can pick the number while the old server is still removing its socket, and a
# window started then fails with "Failed to initialize X11 client". Here the free number is chosen from lock and socket
# together (from 100 up, never :0, the desktop), and the server has exited, socket and lock removed, before this
# returns. Callers hold the gate lock, so no other start races for a number.
run_on_private_display() {
  local geometry=$1 number=100 waited=0 server status=0
  shift
  while [[ -e /tmp/.X11-unix/X$number || -e /tmp/.X$number-lock ]]; do number=$((number + 1)); done
  Xvfb ":$number" -screen 0 "$geometry" -nolisten tcp >/dev/null 2>&1 &
  server=$!
  until [[ -S /tmp/.X11-unix/X$number ]]; do
    if ! kill -0 "$server" 2>/dev/null || ((waited > 100)); then
      echo "private-display: Xvfb :$number did not start" >&2
      kill "$server" 2>/dev/null || true
      wait "$server" 2>/dev/null || true
      return 1
    fi
    sleep 0.1
    waited=$((waited + 1))
  done
  env DISPLAY=":$number" "$@" || status=$?
  kill "$server" 2>/dev/null || true
  wait "$server" 2>/dev/null || true
  return "$status"
}
