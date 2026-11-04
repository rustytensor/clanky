#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
build="${CLANKY_PACKAGE_BUILD_DIR:-${CARGO_TARGET_DIR:-$root/target}/packaging}"
export CLANKY_PACKAGE_BUILD_DIR="$build"
lock="${CLANKY_BUILD_LOCK:-${CARGO_TARGET_DIR:-$root/target}/.build.lock}"
export CLANKY_BUILD_LOCK="$lock"
export CARGO_TARGET_DIR="$build/target"
: "${CARGO_HOME:?set CARGO_HOME}"
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=8
mkdir -p "$build"
work=$(mktemp -d "$build/gate.XXXXXXXX")
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: Linux packaging (exit $rc)" >&2; fi' EXIT
mkdir -p "$work/desktop" "$work/rpmbuild"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
cp packaging/fedora/clanky.spec "$work/clanky.spec"
if [[ ${CLANKY_GATE_FAULT:-} == version ]]; then
    python3 - "$work/clanky.spec" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
s, count = re.subn(r'^Version:\s*\S+', 'Version: 0.0.0', p.read_text(), flags=re.MULTILINE)
assert count == 1
p.write_text(s)
PY
fi
version=$(python3 scripts/packaging-version.py)
tarball="$work/rpmbuild/SOURCES/clanky-$version.tar.gz"
python3 scripts/packaging-version.py --check --spec "$work/clanky.spec" --tarball "$tarball"
cp packaging/linux/clanky.desktop "$work/desktop/clanky.desktop"
if [[ ${CLANKY_GATE_FAULT:-} == desktop ]]; then
    python3 - "$work/desktop/clanky.desktop" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()
lines = s.splitlines(keepends=True)
assert sum(line.startswith('Exec=') for line in lines) == 1
p.write_text(''.join('Exec=clanky-broken\n' if line.startswith('Exec=') else line for line in lines))
PY
fi
check_desktop() {
    timeout 30 python3 - "$1" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
lines = path.read_text().splitlines()
for expected in ('Exec=clanky', 'Icon=io.github.rustytensor.Clanky', 'Name=Clanky'):
    key = expected.split('=', 1)[0] + '='
    observed = [line for line in lines if line.startswith(key)]
    if observed != [expected]:
        print(f'FAIL: {path}: expected {expected!r}; observed {observed!r}', file=sys.stderr)
        sys.exit(1)
PY
}
timeout 30 desktop-file-validate "$work/desktop/clanky.desktop"
check_desktop "$work/desktop/clanky.desktop"
timeout 30 appstreamcli validate --no-net packaging/linux/io.github.rustytensor.Clanky.metainfo.xml
timeout 1800 "$root/scripts/vendor-tarball.sh" "$tarball"
timeout 1800 flock "$lock" rpmbuild -bb \
    --define "_topdir $work/rpmbuild" --define '_smp_mflags -j8' \
    packaging/fedora/clanky.spec
mapfile -t rpms < <(find "$work/rpmbuild/RPMS" -type f -name '*.rpm' ! -name '*debuginfo*' ! -name '*debugsource*')
[[ ${#rpms[@]} == 1 ]] || { echo 'Expected exactly one binary RPM' >&2; exit 1; }
mkdir -p "$work/extracted"
(cd "$work/extracted" && timeout 30 rpm2cpio "${rpms[0]}" | timeout 30 cpio -id --quiet ./usr/share/applications/clanky.desktop)
check_desktop "$work/extracted/usr/share/applications/clanky.desktop"
rpm -qlp "${rpms[0]}" | LC_ALL=C sort > "$work/all-files"
rpm -qp --qf '[%{FILENAMES}\t%{FILEMODES}\t%{FILELINKTOS}\n]' "${rpms[0]}" > "$work/file-metadata"
python3 - "$work/all-files" "$work/file-metadata" "$work/actual-files" <<'PY'
import pathlib, posixpath, re, stat, sys
all_paths = pathlib.Path(sys.argv[1]).read_text().splitlines()
metadata = {}
for row in pathlib.Path(sys.argv[2]).read_text().splitlines():
    path, mode, target = row.split('\t', 2)
    if path in metadata:
        sys.exit(f'FAIL: duplicate RPM metadata path: {path}')
    metadata[path] = (int(mode), target)
pattern = re.compile(r'^/usr/lib/\.build-id(/[0-9a-f]{2}(/[0-9a-f]{38}(\.debug)?)?)?$')
remaining = []
links = 0
for path in all_paths:
    if not pattern.fullmatch(path):
        remaining.append(path)
        continue
    mode, target = metadata[path]
    if path.count('/') < 5:
        if not stat.S_ISDIR(mode):
            sys.exit(f'FAIL: build-ID directory is not a directory: {path}')
    else:
        resolved = posixpath.normpath(posixpath.join(posixpath.dirname(path), target))
        if not stat.S_ISLNK(mode) or resolved != '/usr/bin/clanky':
            sys.exit(f'FAIL: build-ID link {path} targets {target!r}, expected /usr/bin/clanky')
        links += 1
if not links:
    sys.exit('FAIL: no build-ID link to /usr/bin/clanky')
pathlib.Path(sys.argv[3]).write_text(''.join(path + '\n' for path in remaining))
PY
cat > "$work/expected-files" <<'EOF'
/usr/bin/clanky
/usr/share/applications/clanky.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
/usr/share/licenses/clanky
/usr/share/licenses/clanky/LICENSE-APACHE
/usr/share/licenses/clanky/LICENSE-MIT
/usr/share/man/man1/clanky.1.gz
/usr/share/metainfo/io.github.rustytensor.Clanky.metainfo.xml
EOF
diff -u "$work/expected-files" "$work/actual-files"
rpmlint packaging/fedora/clanky.spec "${rpms[0]}"
srpm=$(timeout 1800 "$root/scripts/build-srpm.sh" "$work/srpm-output")
actual_version=$(rpm -qp --qf '%{VERSION}' "$srpm")
[[ $actual_version == "$version" ]] || { echo "FAIL: SRPM version $actual_version, expected $version" >&2; exit 1; }
rpm -qpl "$srpm" > "$work/srpm-files"
python3 - "$work/srpm-files" "$version" <<'PY'
import pathlib, sys
files = pathlib.Path(sys.argv[1]).read_text().splitlines()
expected = f'clanky-{sys.argv[2]}.tar.gz'
if files.count(expected) != 1:
    sys.exit(f'FAIL: SRPM expected source {expected!r}; observed {files!r}')
PY
echo "SRPM: $srpm"
echo "RPM: ${rpms[0]}"
echo 'PASS: Linux packaging'
