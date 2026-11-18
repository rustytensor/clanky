#!/usr/bin/env bash
# Sourced by the gate scripts that run many independent simulated gate processes (--faults): start_job NAME COMMAND...
# runs COMMAND in the background at low priority, with at most JOB_LIMIT running at once, and keeps its output and
# exit status under JOB_DIR; take_job NAME waits for every started job and sets `output` and `status` from NAME's.
# The caller reads the results in its own order, so what it prints is the same as when it ran them one after another.
JOB_LIMIT=${JOB_LIMIT:-4}

start_job() {
  local name=$1
  shift
  while (($(jobs -rp | wc -l) >= JOB_LIMIT)); do
    wait -n || true # a job's failure is its own status file, not this loop's
  done
  (
    code=0
    nice -n 10 "$@" >"$JOB_DIR/$name.out" 2>&1 || code=$?
    echo "$code" >"$JOB_DIR/$name.status"
  ) &
}

# output and status are read by the caller that sourced this file.
# shellcheck disable=SC2034
take_job() {
  take_jobs "$1"
}

# take_jobs NAME...: the same for several jobs that together make one run: `output` is their outputs one after another,
# `status` is 0 only when every one of them succeeded.
# shellcheck disable=SC2034
take_jobs() {
  local name code
  wait
  output=
  status=0
  for name in "$@"; do
    output+=$(<"$JOB_DIR/$name.out")$'\n'
    code=$(<"$JOB_DIR/$name.status")
    ((code == 0)) || status=$code
  done
}
