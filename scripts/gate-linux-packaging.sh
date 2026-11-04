#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
build=/srv/devops/build/clanky-pkg
export CARGO_TARGET_DIR="$build/target"
export CARGO_HOME=/srv/devops/cache/clanky-cargo
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=8
mkdir -p "$build"
work=$(mktemp -d "$build/gate.XXXXXXXX")
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: Linux packaging (exit $rc)" >&2; fi' EXIT
mkdir -p "$work/desktop" "$work/rpmbuild"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
cp packaging/linux/clanky.desktop "$work/desktop/clanky.desktop"
if [[ ${CLANKY_GATE_FAULT:-} == desktop ]]; then
    python3 - "$work/desktop/clanky.desktop" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()
lines = s.splitlines(keepends=True)
assert sum(line.startswith('Exec=') for line in lines) == 1
p.write_text(''.join('Exec=\n' if line.startswith('Exec=') else line for line in lines))
PY
fi
desktop-file-validate "$work/desktop/clanky.desktop"
appstreamcli validate --no-net packaging/linux/io.github.rustytensor.Clanky.metainfo.xml
"$root/scripts/vendor-tarball.sh" "$work/rpmbuild/SOURCES/clanky-0.3.3.tar.gz"
flock /srv/devops/build/clanky/.build.lock rpmbuild -bb \
    --define "_topdir $work/rpmbuild" --define '_smp_mflags -j8' \
    packaging/fedora/clanky.spec
mapfile -t rpms < <(find "$work/rpmbuild/RPMS" -type f -name '*.rpm' ! -name '*debuginfo*' ! -name '*debugsource*')
[[ ${#rpms[@]} == 1 ]] || { echo 'Expected exactly one binary RPM' >&2; exit 1; }
rpm -qlp "${rpms[0]}" | LC_ALL=C sort > "$work/actual-files"
cat > "$work/expected-files" <<'EOF'
/usr/bin/clanky
/usr/share/applications/clanky.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
/usr/share/licenses/clanky/LICENSE-APACHE
/usr/share/licenses/clanky/LICENSE-MIT
/usr/share/man/man1/clanky.1.gz
/usr/share/metainfo/io.github.rustytensor.Clanky.metainfo.xml
EOF
diff -u "$work/expected-files" "$work/actual-files"
rpmlint packaging/fedora/clanky.spec "${rpms[0]}"
echo "RPM: ${rpms[0]}"
echo 'PASS: Linux packaging'
