#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 1 || $1 != /* ]]; then
    echo 'Usage: vendor-tarball.sh /absolute/path/clanky-VERSION.tar.gz' >&2
    exit 2
fi
output=$1
root=$(git rev-parse --show-toplevel)
cd "$root"
version=$(python3 scripts/packaging-version.py --check --tarball "$output")
build="${CLANKY_PACKAGE_BUILD_DIR:-${CARGO_TARGET_DIR:-$root/target}/packaging}"
: "${CARGO_HOME:?set CARGO_HOME}"
lock="${CLANKY_BUILD_LOCK:-${CARGO_TARGET_DIR:-$root/target}/.build.lock}"
export CARGO_TARGET_DIR="$build/target"
export CARGO_NET_OFFLINE=true
mkdir -p "$build"
work=$(mktemp -d "$build/vendor.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/clanky-$version"
git archive HEAD | tar -x -C "$work/clanky-$version"
cd "$work/clanky-$version"
archive_version=$(python3 scripts/packaging-version.py --check --tarball "$output")
[[ $archive_version == "$version" ]] || { echo "FAIL: committed Cargo version $archive_version differs from workspace $version" >&2; exit 1; }
mkdir -p .cargo
flock "$lock" cargo vendor --locked --offline vendor > .cargo/config.toml
mkdir -p "$(dirname "$output")"
tar -czf "$output" -C "$work" "clanky-$version"
