#!/usr/bin/env bash
# G5's parity gate: the new binary does everything the old one did, and the public repository builds on its own.
#
#   scripts/gate-parity.sh [--packages] [--only STEP[,STEP...]] [--faults]
#
# Steps, in this order, one "ok:" or "FAIL:" line each, then PASS or FAIL:
#   lists       every gate- and check- script is on one list of scripts/parity (check_lists.py)
#   build       clanky-gpui, release, into this gate's own target folder
#   gates       every command of scripts/parity/gates.txt, each with a time limit and its own log
#   deps        no workspace crate depends on winit, cosmic-text or the old renderer (check_dependencies.py)
#   bundle      third_party/gpui-ce equals our copy of gpui-ce at the commit in its SOURCE file
#   fresh-clone a clean clone of this branch builds with no network, passes its unit tests and opens a window
#   packages    the .deb and Arch packages build and install in clean containers (only with --packages)
#   speed       G1's speed numbers on the full window, against baseline.json
# PASS: parity needs every step; --packages left out ends in "PASS: parity, packages left out". --only runs the named
# steps and ends in "PASS: parity steps ...". Logs go to LOG_DIR (default: a new folder under /srv/devops/build).
#
# --faults needs no binary and no display. It runs the checks of these steps on scratch trees and each planted fault
#   must fail the check aimed at it: one bundled file edited by hand, one crate left out, a git source on our server
#   left in Cargo.lock, a winit dependency in a scratch crate, and one applying gate removed from gates.txt while its
#   script stays.
# Environment: CARGO_HOME (default /srv/devops/cache/clanky-cargo, the crates.io cache), CARGO_TARGET_DIR (default
#   /srv/devops/build/clanky-parity-target), GATE_TIMEOUT (seconds per listed gate, default 3600).
set -euo pipefail
cd "$(dirname "$0")/.."

PARITY_DIR=scripts/parity
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/srv/devops/build/clanky-parity-target}
BUILD_LOCK=${CLANKY_BUILD_LOCK:-/srv/devops/build/clanky/.build.lock}
GATE_LOCK=${CLANKY_GATE_LOCK:-/srv/devops/build/clanky/.gate.lock}
GATE_TIMEOUT=${GATE_TIMEOUT:-3600}
ALL_STEPS=(lists build gates deps bundle fresh-clone packages speed)

with_packages=0
faults=0
only=()
while (($#)); do
  case $1 in
    --packages) with_packages=1 ;;
    --faults) faults=1 ;;
    --only) shift; IFS=, read -ra only <<<"${1:?--only needs step names}" ;;
    *) echo "usage: scripts/gate-parity.sh [--packages] [--only STEP[,STEP...]] [--faults]" >&2; exit 2 ;;
  esac
  shift
done

failed=0
ok() { echo "ok: $*"; }
fail() { echo "FAIL: $*"; failed=1; }
wanted() {
  local step
  ((${#only[@]})) || return 0
  for step in "${only[@]}"; do [[ $step == "$1" ]] && return 0; done
  return 1
}
# last_line FILE: the last non-empty line of a log, for the one-line summary of a step.
last_line() { grep -v '^[[:space:]]*$' "$1" | tail -n 1 | cut -c1-300; }

# bundle_pin: the commit of gpui-ce this tree is built from: the bundle's SOURCE file, or the git dependency before
# the first bundling.
bundle_pin() {
  if [[ -f third_party/gpui-ce/SOURCE ]]; then
    sed -n 's/^rev=//p' third_party/gpui-ce/SOURCE
  else
    sed -n 's/.*gpui-ce.*rev = "\([0-9a-f]\{40\}\)".*/\1/p' crates/clanky-gpui/Cargo.toml | head -n 1
  fi
}

# ----------------------------------------------------------------------------------------------- --faults
if ((faults)); then
  scratch=$(mktemp -d /srv/devops/build/parity-faults.XXXXXX)
  trap 'rm -rf -- "${scratch:?}"' EXIT
  caught=0
  expect_failure() { # LABEL WORDS COMMAND...: the command must fail and its output must contain WORDS
    local label=$1 words=$2 output
    shift 2
    if output=$("$@" 2>&1); then
      fail "planted fault not caught: $label"
    elif [[ $output == *"$words"* ]]; then
      echo "CAUGHT: $label: $(grep -m1 -F "$words" <<<"$output" | cut -c1-200)"
      caught=$((caught + 1))
    else
      fail "$label failed for another reason: $(tail -n 1 <<<"$output" | cut -c1-200)"
    fi
  }
  # The lists: a good tree passes, then one applying gate leaves gates.txt while its script stays.
  mkdir -p "$scratch/lists/scripts/parity"
  cp "$PARITY_DIR"/{gates,retired,other}.txt "$scratch/lists/scripts/parity/"
  for entry in scripts/gate-* scripts/check-*; do : >"$scratch/lists/$entry"; done
  python3 "$PARITY_DIR/check_lists.py" --root "$scratch/lists" >/dev/null || fail "the good lists are rejected"
  victim=$(grep -v '^#' "$PARITY_DIR/gates.txt" | head -n 1 | cut -f1)
  grep -v "^$victim"$'\t' "$PARITY_DIR/gates.txt" >"$scratch/lists/scripts/parity/gates.txt"
  expect_failure "$victim removed from gates.txt while its script stays" "$victim" \
    python3 "$PARITY_DIR/check_lists.py" --root "$scratch/lists"
  # The dependency check: a scratch workspace, good, then with a crate that depends on winit.
  mkdir -p "$scratch/deps/good/src" "$scratch/deps/bad/src"
  printf '[workspace]\nmembers = ["good"]\nresolver = "3"\n' >"$scratch/deps/Cargo.toml"
  printf '[package]\nname = "good"\nversion = "0.0.0"\nedition = "2024"\n' >"$scratch/deps/good/Cargo.toml"
  printf '[package]\nname = "bad"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\nwinit = "=0.30.13"\n' >"$scratch/deps/bad/Cargo.toml"
  : >"$scratch/deps/good/src/lib.rs"
  : >"$scratch/deps/bad/src/lib.rs"
  python3 "$PARITY_DIR/check_dependencies.py" --root "$scratch/deps" >/dev/null || fail "the good scratch workspace is rejected"
  sed -i 's/members = \["good"\]/members = ["good", "bad"]/' "$scratch/deps/Cargo.toml"
  expect_failure "a winit import in a scratch crate" "bad depends on winit" \
    python3 "$PARITY_DIR/check_dependencies.py" --root "$scratch/deps"
  # The bundle: a scratch copy of the committed tree, bundled now, then the check's own three faults.
  pin=$(bundle_pin)
  [[ -n $pin ]] || { fail "no gpui-ce commit found in third_party/gpui-ce/SOURCE or crates/clanky-gpui/Cargo.toml"; pin=none; }
  if [[ $pin != none ]]; then
    mkdir -p "$scratch/tree"
    git archive HEAD | tar -x -C "$scratch/tree"
    (cd "$scratch/tree" && scripts/bundle-gpui-ce.sh "$pin" >/dev/null 2>"$scratch/bundle.err") ||
      fail "bundling in the scratch tree failed: $(tail -n 1 "$scratch/bundle.err")"
    python3 "$scratch/tree/scripts/check-gpui-ce-bundle.py" --root "$scratch/tree" >/dev/null || fail "the good scratch bundle is rejected"
    if output=$(python3 "$scratch/tree/scripts/check-gpui-ce-bundle.py" --root "$scratch/tree" --faults 2>&1); then
      echo "$output" | grep '^CAUGHT:' | cut -c1-200
      caught=$((caught + $(grep -c '^CAUGHT:' <<<"$output")))
    else
      fail "the bundle check's planted faults: $(tail -n 1 <<<"$output" | cut -c1-200)"
    fi
  fi
  if ((failed || caught != 5)); then
    echo "FAIL: parity checks catch $caught of 5 planted faults"
    exit 1
  fi
  echo "PASS: parity checks catch all 5 planted faults"
  exit 0
fi

# ----------------------------------------------------------------------------------------------- steps
LOG_DIR=${LOG_DIR:-$(mktemp -d /srv/devops/build/parity-run.XXXXXX)}
mkdir -p "$LOG_DIR"
step_lists() {
  if line=$(python3 "$PARITY_DIR/check_lists.py"); then ok "lists: ${line#PASS: }"; else fail "lists: ${line#FAIL: }"; fi
}

step_build() {
  if flock "$BUILD_LOCK" cargo build --offline --locked --release -j 8 -p clanky-gpui >"$LOG_DIR/build.log" 2>&1; then
    ok "build: clanky-gpui in $CARGO_TARGET_DIR/release"
  else
    fail "build: clanky-gpui did not build: $(last_line "$LOG_DIR/build.log")"
  fi
}

step_gates() {
  local stem command covers log status
  export CLANKY_GPUI_BIN=$CARGO_TARGET_DIR/release/clanky-gpui
  while IFS=$'\t' read -r stem command covers; do
    [[ -z $stem || $stem == \#* ]] && continue
    log=$LOG_DIR/gate-$stem.log
    status=0
    timeout -k 30 "$GATE_TIMEOUT" bash -c "$command" >"$log" 2>&1 || status=$?
    if ((status == 0)); then
      ok "gate $stem: $(last_line "$log")"
    elif ((status == 124)); then
      fail "gate $stem: no result in ${GATE_TIMEOUT}s ($log)"
    else
      fail "gate $stem: exit $status: $(last_line "$log") ($log)"
    fi
    : "$covers"
  done <"$PARITY_DIR/gates.txt"
}

step_deps() {
  if line=$(python3 "$PARITY_DIR/check_dependencies.py"); then ok "deps: ${line#PASS: }"; else fail "deps: ${line#FAIL: }"; fi
}

step_bundle() {
  if [[ ! -d third_party/gpui-ce ]]; then
    fail "bundle: third_party/gpui-ce does not exist; run scripts/bundle-gpui-ce.sh and commit the result"
  elif line=$(python3 scripts/check-gpui-ce-bundle.py 2>&1); then
    ok "bundle: ${line#PASS: }"
  else
    fail "bundle: ${line#FAIL: }"
  fi
}

# A clean clone of this branch, built the way a stranger builds it: no network for cargo, no ssh keys, a cargo home with
# only the crates.io cache. The unit tests run, then the window opens on a private X display.
step_fresh_clone() {
  local scratch home log=$LOG_DIR/fresh-clone.log target=$CARGO_TARGET_DIR/fresh-clone
  scratch=$(mktemp -d /srv/devops/build/parity-clone.XXXXXX)
  home=$scratch/home
  mkdir -p "$home/.ssh" "$home/cargo"
  chmod 700 "$home/.ssh"
  ln -s "$CARGO_HOME/registry" "$home/cargo/registry"
  if ! git clone -q "$PWD" "$scratch/tree" >"$log" 2>&1; then
    fail "fresh-clone: cannot clone this branch: $(last_line "$log")"
    rm -rf -- "${scratch:?}"
    return
  fi
  run_offline() {
    (cd "$scratch/tree" && env -i PATH="$PATH" HOME="$home" CARGO_HOME="$home/cargo" CARGO_NET_OFFLINE=true \
      CARGO_NET_GIT_FETCH_WITH_CLI=false CARGO_TARGET_DIR="$target" flock "$BUILD_LOCK" cargo "$@") >>"$log" 2>&1
  }
  if ! run_offline build --offline --locked -j 8 -p clanky-gpui; then
    fail "fresh-clone: the offline build failed: $(last_line "$log")"
  elif ! run_offline test --offline --locked -j 8 --workspace; then
    fail "fresh-clone: the unit tests failed: $(last_line "$log")"
  elif ! CLANKY_GPUI_BIN=$target/debug/clanky-gpui flock -o "$GATE_LOCK" bash "$scratch/tree/scripts/terminal-pane/check_x11_class.sh" >>"$log" 2>&1; then
    fail "fresh-clone: the window did not open on Xvfb: $(last_line "$log")"
  else
    ok "fresh-clone: built offline, unit tests pass, the window opens on Xvfb"
  fi
  rm -rf -- "${scratch:?}"
}

step_packages() {
  if ((!with_packages)); then
    echo "left out: packages (run with --packages)"
    return
  fi
  local log=$LOG_DIR/packages.log
  if timeout -k 30 "$GATE_TIMEOUT" bash scripts/gate-p1-packages.sh >"$log" 2>&1; then
    ok "packages: $(last_line "$log")"
  else
    fail "packages: $(last_line "$log") ($log)"
  fi
}

step_speed() {
  local binary=$CARGO_TARGET_DIR/release/clanky-gpui log=$LOG_DIR/speed.log runs limit_time limit_key limit_start
  runs=$(sed -n 's/^SPEED_RUNS=\([0-9]*\).*/\1/p' scripts/gate-terminal-pane.sh)
  limit_time=$(sed -n 's/^SPEED_LIMIT_TIME=\([0-9.]*\).*/\1/p' scripts/gate-terminal-pane.sh)
  limit_key=$(sed -n 's/^SPEED_LIMIT_KEY_MS=\([0-9.]*\).*/\1/p' scripts/gate-terminal-pane.sh)
  limit_start=$(sed -n 's/^SPEED_LIMIT_START=\([0-9.]*\).*/\1/p' scripts/gate-terminal-pane.sh)
  if [[ -z $runs || -z $limit_time || -z $limit_key || -z $limit_start ]]; then
    fail "speed: cannot read the limits from scripts/gate-terminal-pane.sh"
  elif [[ ! -x $binary ]]; then
    fail "speed: no $binary"
  elif ! bash scripts/terminal-pane/speed.sh "$binary" --runs "$runs" --out "$LOG_DIR/speed.json" >"$log" 2>&1; then
    fail "speed: the harness failed: $(last_line "$log")"
  elif verdict=$(python3 "$PARITY_DIR/speed_verdict.py" "$LOG_DIR/speed.json" scripts/terminal-pane/baseline.json "$limit_time" "$limit_key" "$limit_start"); then
    ok "speed: $verdict (load $(cut -d' ' -f1 /proc/loadavg))"
  else
    fail "speed: $verdict (load $(cut -d' ' -f1 /proc/loadavg))"
  fi
}

ran=()
for step in "${ALL_STEPS[@]}"; do
  wanted "$step" || continue
  ran+=("$step")
  "step_${step//-/_}"
done

echo "logs: $LOG_DIR"
if ((failed)); then
  echo "FAIL: parity"
  exit 1
fi
if ((${#only[@]})); then
  echo "PASS: parity steps ${ran[*]}"
elif ((!with_packages)); then
  echo "PASS: parity, packages left out"
else
  echo "PASS: parity"
fi
