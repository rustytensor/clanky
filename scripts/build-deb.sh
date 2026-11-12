#!/usr/bin/env bash
# Build clanky_<version>-1_amd64.deb for Debian 13 and Ubuntu 26.04 into OUTDIR and print its path.
# The binary is built inside a Debian 13 Rust container from the offline vendored tarball, so it
# links trixie's glibc and cargo needs no network. apt inside the container fetches build packages.
set -euo pipefail
if [[ $# != 1 || $1 != /* ]]; then
    echo 'Usage: build-deb.sh /absolute/output-directory' >&2
    exit 2
fi
output=$1
root=$(git rev-parse --show-toplevel)
cd "$root"
version=$(python3 scripts/packaging-version.py --check)
image=${CLANKY_DEB_IMAGE:-docker.io/library/rust:1.98-slim-trixie}
export CARGO_HOME=${CARGO_HOME:-/srv/devops/cache/clanky-cargo}
build="${CLANKY_PACKAGE_BUILD_DIR:-${CARGO_TARGET_DIR:-$root/target}/packaging}"
mkdir -p "$build" "$output"
destination="$output/clanky_${version}-1_amd64.deb"
[[ ! -e $destination ]] || { echo "FAIL: refusing to overwrite $destination" >&2; exit 1; }
work=$(mktemp -d "$build/deb.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
# Kept between runs so a rebuild of the same sources is incremental.
cache="$build/deb-target"
mkdir -p "$cache"

"$root/scripts/vendor-tarball.sh" "$work/clanky-$version.tar.gz" >&2

podman run --rm \
    -v "$work:/work:Z" -v "$cache:/target:Z" \
    -e VERSION="$version" -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS=8 -e CARGO_NET_OFFLINE=true \
    "$image" bash -euo pipefail -c '
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends libsqlite3-dev pkg-config dpkg-dev binutils >/dev/null
cd /tmp && tar -xzf /work/clanky-$VERSION.tar.gz && cd clanky-$VERSION
cargo build --release --frozen --offline -j 8 -p clanky --bin clanky >&2
pkg=/work/pkg
install -Dm755 /target/release/clanky $pkg/usr/bin/clanky
strip --strip-unneeded --remove-section=.comment --remove-section=.note $pkg/usr/bin/clanky
install -Dm644 packaging/linux/clanky.desktop $pkg/usr/share/applications/clanky.desktop
install -Dm644 packaging/linux/io.github.rustytensor.Clanky.metainfo.xml \
    $pkg/usr/share/metainfo/io.github.rustytensor.Clanky.metainfo.xml
install -Dm644 packaging/linux/io.github.rustytensor.Clanky.svg \
    $pkg/usr/share/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
install -d $pkg/usr/share/man/man1 $pkg/usr/share/doc/clanky $pkg/DEBIAN
gzip -9n < packaging/linux/clanky.1 > $pkg/usr/share/man/man1/clanky.1.gz
sh packaging/debian/copyright.sh > $pkg/usr/share/doc/clanky/copyright
chmod 0644 $pkg/usr/share/man/man1/clanky.1.gz $pkg/usr/share/doc/clanky/copyright

# Depends from the libraries the binary links (dpkg-shlibdeps reads its ELF NEEDED entries).
mkdir -p /tmp/shlib/debian && printf "Source: clanky\n\nPackage: clanky\nArchitecture: any\n" > /tmp/shlib/debian/control
shlibs=$(cd /tmp/shlib && dpkg-shlibdeps -O $pkg/usr/bin/clanky | sed -n "s/^shlibs:Depends=//p")
case $shlibs in *libsqlite3-0*) ;; *) echo "FAIL: binary does not link libsqlite3 ($shlibs)" >&2; exit 1;; esac
sed -e "s/@VERSION@/$VERSION-1/" -e "s/@DEPENDS@/$shlibs/" \
    -e "s/@SIZE@/$(du -sk --exclude=DEBIAN $pkg | cut -f1)/" packaging/debian/control.in > $pkg/DEBIAN/control
(cd $pkg && find usr -type f -print0 | LC_ALL=C sort -z | xargs -0 md5sum) > $pkg/DEBIAN/md5sums
chmod 0644 $pkg/DEBIAN/control $pkg/DEBIAN/md5sums
find $pkg -type d -exec chmod 0755 {} +
dpkg-deb --root-owner-group -Zxz -b $pkg /work/clanky.deb >&2
' >&2

cp -- "$work/clanky.deb" "$destination"
printf '%s\n' "$destination"
