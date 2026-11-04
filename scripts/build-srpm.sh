#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 1 || $1 != /* ]]; then
    echo 'Usage: build-srpm.sh /absolute/output-directory' >&2
    exit 2
fi
output=$1
root=$(git rev-parse --show-toplevel)
cd "$root"
version=$(python3 scripts/packaging-version.py --check)
build=/srv/devops/build/clanky-pkg
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CARGO_NET_OFFLINE=true
mkdir -p "$build" "$output"
work=$(mktemp -d "$build/srpm.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
tarball="$work/SOURCES/clanky-$version.tar.gz"
"$root/scripts/vendor-tarball.sh" "$tarball"
timeout 1800 rpmbuild -bs --define "_topdir $work" "$root/packaging/fedora/clanky.spec" >&2
mapfile -t packages < <(find "$work/SRPMS" -maxdepth 1 -type f -name '*.src.rpm')
[[ ${#packages[@]} == 1 ]] || { echo 'FAIL: expected one source RPM' >&2; exit 1; }
destination="$output/$(basename "${packages[0]}")"
[[ ! -e $destination ]] || { echo "FAIL: refusing to overwrite $destination" >&2; exit 1; }
cp -- "${packages[0]}" "$destination"
printf '%s\n' "$destination"
