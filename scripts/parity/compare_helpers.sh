#!/usr/bin/env bash
# The gate helper programs, old build against new build, with the same input: standard output, standard error and exit
# status must be byte for byte the same.
#
#   scripts/parity/compare_helpers.sh OLD_DIR NEW_DIR
#
# Each directory holds clanky-theme-gate, clanky-postai-gate, polish-session-gate and the same clanky server binary.
# Prints one "identical:" or "DIFFERENT:" line per case, then PASS or FAIL.
set -euo pipefail
cd "$(dirname "$0")/../.."
old=$(realpath "${1:?OLD_DIR}")
new=$(realpath "${2:?NEW_DIR}")
scratch=$(mktemp -d /srv/devops/build/compare-helpers.XXXXXX)
trap 'rm -rf -- "${scratch:?}"' EXIT
failed=0

# run_case NAME DIRECTORY COMMAND...: runs one build of one helper; leaves NAME.out, NAME.err and NAME.status in
# the scratch folder of that build.
run_case() {
  local name=$1 dir=$2 status=0
  shift 2
  mkdir -p "$scratch/$(basename "$dir")"
  "$@" >"$scratch/$(basename "$dir")/$name.out" 2>"$scratch/$(basename "$dir")/$name.err" || status=$?
  echo "$status" >"$scratch/$(basename "$dir")/$name.status"
}

# same NAME: compares the three files of one case between the two builds.
same() {
  local name=$1 file
  for file in out err status; do
    if ! cmp -s "$scratch/$(basename "$old")/$name.$file" "$scratch/$(basename "$new")/$name.$file"; then
      echo "DIFFERENT: $name ($file)"
      diff "$scratch/$(basename "$old")/$name.$file" "$scratch/$(basename "$new")/$name.$file" | head -n 6
      failed=1
      return
    fi
  done
  echo "identical: $name ($(wc -c <"$scratch/$(basename "$old")/$name.out") bytes out, $(wc -c <"$scratch/$(basename "$old")/$name.err") bytes err, exit $(cat "$scratch/$(basename "$old")/$name.status"))"
}

[[ $(basename "$old") != "$(basename "$new")" ]] || { echo "FAIL: the two folders need different names" >&2; exit 2; }
export CARGO_TARGET_DIR=$scratch/target
mkdir -p "$CARGO_TARGET_DIR"

for dir in "$old" "$new"; do
  run_case theme-palettes "$dir" "$dir/clanky-theme-gate" --palettes
  run_case theme-storage "$dir" "$dir/clanky-theme-gate" --storage
  run_case theme-storage-fault "$dir" env CLANKY_THEME_GATE_FAULT=storage "$dir/clanky-theme-gate" --storage
  run_case theme-no-arguments "$dir" "$dir/clanky-theme-gate"
  run_case theme-bad-argument "$dir" "$dir/clanky-theme-gate" --nonsense
  run_case postai "$dir" "$dir/clanky-postai-gate"
  run_case polish-session "$dir" python3 scripts/gate-polish-session.py "$dir/polish-session-gate" "$dir/clanky"
  run_case polish-bad-arguments "$dir" "$dir/polish-session-gate" nonsense
done
for name in theme-palettes theme-storage theme-storage-fault theme-no-arguments theme-bad-argument postai polish-session polish-bad-arguments; do
  same "$name"
done
if ((failed)); then
  echo "FAIL: a helper's output differs between the old and the new build"
  exit 1
fi
echo "PASS: every helper prints the same bytes and exits the same in the old and the new build"
