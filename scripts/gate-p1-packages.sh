#!/usr/bin/env bash
# P1 gate: a .deb for Debian 13 and Ubuntu 26.04, and an Arch PKGBUILD, each installed in a clean container.
# Prints "PASS: P1 packages" or exits non-zero with FAIL lines.
#
# Interface the packaging work must provide (the gate only calls these):
#   scripts/build-deb.sh OUTDIR    builds clanky_<version>-1_amd64.deb into OUTDIR (empty, absolute) and prints its path
#   packaging/arch/PKGBUILD        pkgname=clanky, pkgver=<Cargo version>, pkgrel=1; source clanky-$pkgver.tar.gz
#
# Expected values, fixed before the work starts:
#   version         the workspace version in Cargo.toml (scripts/packaging-version.py)
#   .deb version    <version>-1, architecture amd64
#   .deb files      exactly the list in expect_deb_files below
#   in each distro  the package installs with the distro's own dependencies; `ldd /usr/bin/clanky` has no
#                   "not found" and links libsqlite3.so.0; `clanky history path` with HOME=/tmp/h prints
#                   /tmp/h/.local/share/clanky/history.sqlite3; `clanky history list` prints "No saved commands."
#   Arch            `pacman -Q clanky` prints "clanky <version>-1"; namcap reports no error (E:) lines
#
# --faults plants three faults, one at a time, and passes only if each is caught:
#   deb-depends  libsqlite3 dependency removed from the .deb      -> caught by install/ldd in Debian 13
#   deb-files    the man page removed from the .deb               -> caught by the file list
#   arch-version pkgver changed to 0.0.0 in the PKGBUILD          -> caught by pacman -Q
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
version=$(python3 scripts/packaging-version.py)
work=${CLANKY_P1_WORK:-/srv/devops/build/clanky-p1-gate}
fault=${CLANKY_P1_FAULT:-}
DEBIAN=docker.io/library/debian:13
UBUNTU=docker.io/library/ubuntu:26.04
ARCH=docker.io/library/archlinux:latest

if [[ ${1:-} == --faults ]]; then
    caught=0
    for f in deb-depends deb-files arch-version; do
        if CLANKY_P1_FAULT=$f "$0" > "$work-fault-$f.log" 2>&1; then
            echo "FAIL: planted fault $f was not caught (log $work-fault-$f.log)"
        else
            echo "CAUGHT: $f ($(grep -m1 '^FAIL' "$work-fault-$f.log" || echo 'non-zero exit'))"
            caught=$((caught + 1))
        fi
    done
    [[ $caught == 3 ]] && { echo 'PASS: P1 gate catches all 3 planted faults'; exit 0; }
    exit 1
fi

fail() { echo "FAIL: $*" >&2; exit 1; }
rm -rf "$work"; mkdir -p "$work/deb" "$work/arch"

# ---- .deb ----------------------------------------------------------------------------------------
deb=$(scripts/build-deb.sh "$work/deb" | tail -n 1)
[[ -f $deb ]] || fail "build-deb.sh printed no .deb path"
[[ $(basename "$deb") == "clanky_${version}-1_amd64.deb" ]] || fail "deb file name $(basename "$deb")"

if [[ $fault == deb-depends || $fault == deb-files ]]; then
    x="$work/deb/x"; mkdir -p "$x"; dpkg-deb -R "$deb" "$x"
    if [[ $fault == deb-depends ]]; then
        sed -i -E 's/(, *)?libsqlite3-0[^,]*//; s/Depends: *, */Depends: /' "$x/DEBIAN/control"
    else
        rm -f "$x/usr/share/man/man1/clanky.1.gz"
        sed -i '/man1\/clanky.1.gz/d' "$x/DEBIAN/md5sums" 2>/dev/null || true
    fi
    dpkg-deb --root-owner-group -b "$x" "$deb" > /dev/null
fi

[[ $(dpkg-deb -f "$deb" Version) == "${version}-1" ]] || fail "deb Version $(dpkg-deb -f "$deb" Version), expected ${version}-1"
[[ $(dpkg-deb -f "$deb" Architecture) == amd64 ]] || fail "deb Architecture $(dpkg-deb -f "$deb" Architecture)"
expect_deb_files() {
    cat <<'EOF'
/usr/bin/clanky
/usr/share/applications/clanky.desktop
/usr/share/doc/clanky/copyright
/usr/share/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
/usr/share/man/man1/clanky.1.gz
/usr/share/metainfo/io.github.rustytensor.Clanky.metainfo.xml
EOF
}
dpkg-deb -c "$deb" | awk '$1 !~ /^d/ {sub(/^\./, "", $6); print $6}' | LC_ALL=C sort > "$work/deb-files"
diff -u <(expect_deb_files) "$work/deb-files" || fail "deb file list differs from the expected list"

probe='set -e
export HOME=/tmp/h; unset XDG_DATA_HOME; mkdir -p $HOME
if ldd /usr/bin/clanky | grep -q "not found"; then echo "FAIL: missing library: $(ldd /usr/bin/clanky | grep "not found")"; exit 1; fi
ldd /usr/bin/clanky | grep -q "libsqlite3.so.0 =>" || { echo "FAIL: not linked to the system libsqlite3"; exit 1; }
p=$(clanky history path); [ "$p" = /tmp/h/.local/share/clanky/history.sqlite3 ] || { echo "FAIL: history path $p"; exit 1; }
l=$(clanky history list); [ "$l" = "No saved commands." ] || { echo "FAIL: history list printed: $l"; exit 1; }
echo PROBE-OK'
for image in "$DEBIAN" "$UBUNTU"; do
    out=$(podman run --rm --network=host -v "$deb:/tmp/clanky.deb:ro,Z" "$image" sh -c \
        "export DEBIAN_FRONTEND=noninteractive; apt-get update -qq >/dev/null && apt-get install -y -qq /tmp/clanky.deb >/dev/null 2>&1 || { echo 'FAIL: apt-get install failed'; exit 1; }; $probe" 2>&1) \
        || fail "$image: $(grep -m1 FAIL <<<"$out" || tail -n 3 <<<"$out")"
    grep -q PROBE-OK <<<"$out" || fail "$image: probe did not finish"
    echo "ok: $image"
done

# ---- Arch ----------------------------------------------------------------------------------------
cp packaging/arch/PKGBUILD "$work/arch/PKGBUILD"
[[ $fault == arch-version ]] && sed -i -E 's/^pkgver=.*/pkgver=0.0.0/' "$work/arch/PKGBUILD"
# The gate builds from this commit's sources, not from a GitHub download: a local tarball under the
# name the PKGBUILD expects, checksums skipped.
pkgver=$(sed -n -E 's/^pkgver=//p' "$work/arch/PKGBUILD")
git archive --prefix="clanky-$pkgver/" -o "$work/arch/clanky-$pkgver.tar.gz" HEAD
out=$(podman run --rm --network=host -v "$work/arch:/src:Z" "$ARCH" bash -c '
set -e
pacman -Syu --noconfirm --needed base-devel namcap >/dev/null 2>&1
useradd -m b; cp -r /src /home/b/pkg; chown -R b /home/b/pkg
namcap /home/b/pkg/PKGBUILD | grep " E: " && { echo "FAIL: namcap errors in PKGBUILD"; exit 1; }
cd /home/b/pkg
echo "b ALL=(ALL) NOPASSWD: /usr/bin/pacman" > /etc/sudoers.d/b
sudo -u b env MAKEFLAGS=-j8 CARGO_BUILD_JOBS=8 makepkg -s --noconfirm --skipchecksums >/tmp/makepkg.log 2>&1 || { tail -n 20 /tmp/makepkg.log; echo "FAIL: makepkg failed"; exit 1; }
pacman -U --noconfirm /home/b/pkg/clanky-*.pkg.tar.zst >/dev/null
echo "PACMAN-Q $(pacman -Q clanky)"
'"$probe" 2>&1) || fail "Arch: $(grep -m1 FAIL <<<"$out" || tail -n 3 <<<"$out")"
grep -q PROBE-OK <<<"$out" || fail "Arch: probe did not finish"
got=$(sed -n 's/^PACMAN-Q //p' <<<"$out")
[[ $got == "clanky ${version}-1" ]] || fail "Arch: pacman -Q printed '$got', expected 'clanky ${version}-1'"
echo "ok: $ARCH"
echo 'PASS: P1 packages'
