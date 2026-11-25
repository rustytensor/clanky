#!/usr/bin/env bash
# Terminal pane gate, engine check: the engine crates, the editor core (clanky-edit) and the window-side state
# (clanky-state) must not depend on gpui-ce, directly or through anything else.
# Prints "ok: engine-free <crate count> crates, no gpui" or "FAIL: engine-free <crate> depends on <gpui crate>"; exit 1 on failure.
# --fault [CRATE] plants the failure: it copies the workspace to a scratch folder, adds gpui-ce to CRATE (default
# clanky-core; the window chrome gate plants it in clanky-state), and runs the same check there.
# Only dependency resolution runs, no build. The fault resolves offline: the window crate pins the same gpui-ce
# commit, so cargo's git cache already holds it. A fetch, if one is ever needed, never waits for a prompt.
set -euo pipefail

ENGINE_CRATES=(clanky-core clanky-vt clanky-vte clanky-api clanky-edit clanky-state)
# The pin and package name are the copy check's own (scripts/copy-check/Cargo.toml in servorra/gpui-ce, branch clanky).
GPUI_DEPENDENCY='gpui_ce = { git = "https://git.servorra.com/servorra/gpui-ce", rev = "f388375d03e63d75811fa1cca3a563203b920d98", package = "gpui-ce" }'

# Scratch copies go in the system temp folder unless CLANKY_GATE_TMP names another; cargo uses its usual home.
GATE_TMP=${CLANKY_GATE_TMP:-${TMPDIR:-/tmp}}

check_workspace() {
  local workspace=$1 offline=$2 crate line name
  local gpui_found=0
  local all_names
  all_names=$(mktemp)
  for crate in "${ENGINE_CRATES[@]}"; do
    local tree
    tree=$(cd "$workspace" && cargo tree ${offline:+--offline} -e normal,build -p "$crate" --prefix none -f '{p}' 2>"$all_names.err") || {
      echo "FAIL: engine-free cargo tree for $crate did not run in $workspace:" >&2
      tail -5 "$all_names.err" >&2
      rm -f "$all_names" "$all_names.err"
      return 1
    }
    while read -r line; do
      name=${line%% *}
      echo "$name" >>"$all_names"
      if [[ $name == gpui* && $gpui_found == 0 ]]; then
        echo "FAIL: engine-free $crate depends on $name"
        gpui_found=1
      fi
    done <<<"$tree"
  done
  if [[ $gpui_found == 0 ]]; then
    echo "ok: engine-free $(sort -u "$all_names" | wc -l) crates, no gpui"
  fi
  rm -f "$all_names" "$all_names.err"
  return "$gpui_found"
}

workspace_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

if [[ ${1:-} == --fault ]]; then
  fault_crate=${2:-clanky-core}
  [[ " ${ENGINE_CRATES[*]} " == *" $fault_crate "* ]] || { echo "FAIL: engine-free --fault: $fault_crate is not a checked crate" >&2; exit 1; }
  scratch=$(mktemp -d "$GATE_TMP/engine-free-fault.XXXXXX")
  trap 'rm -rf "$scratch"' EXIT
  tar -C "$workspace_root" --exclude=.git --exclude=target -cf - . | tar -C "$scratch" -xf -
  sed -i "/^\[dependencies\]/a $GPUI_DEPENDENCY" "$scratch/crates/$fault_crate/Cargo.toml"
  export CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=url.git@git.servorra.com:.insteadOf GIT_CONFIG_VALUE_0=https://git.servorra.com/
  export GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND="ssh -o BatchMode=yes"
  if check_workspace "$scratch" --offline </dev/null; then
    echo "FAIL: engine-free planted fault was not caught" >&2
    exit 1
  fi
  exit 0
fi

check_workspace "$workspace_root" --offline || exit 1
