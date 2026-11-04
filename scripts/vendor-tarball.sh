#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 1 || $1 != /* ]]; then
    echo 'Usage: vendor-tarball.sh /absolute/path/clanky-0.3.3.tar.gz' >&2
    exit 2
fi
output=$1
root=$(git rev-parse --show-toplevel)
cd "$root"
version=0.3.3
build=/srv/devops/build/clanky-pkg
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CARGO_TARGET_DIR="$build/target"
export CARGO_NET_OFFLINE=true
mkdir -p "$build"
work=$(mktemp -d "$build/vendor.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/clanky-$version"
git archive HEAD | tar -x -C "$work/clanky-$version"
cd "$work/clanky-$version"
mkdir -p .cargo
cargo vendor --locked --offline vendor > .cargo/config.toml
mkdir -p "$(dirname "$output")"
tar -czf "$output" -C "$work" "clanky-$version"
