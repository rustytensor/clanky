#!/bin/bash
# Gate for scripts/bump-version.py, on a scratch clone of HEAD (tags stay in the clone) (with the bump and check scripts of this tree).
# Usage: scripts/gate-bump-version.sh [--faults]
# Expected results, fixed before the run (start: 1.7.0 everywhere):
#   B1 1.7.1-dev.1: Cargo.toml and Cargo.lock (package clanky) 1.7.1-dev.1; agent-api "1.7.1-dev.1"; PKGBUILD and
#      .SRCINFO pkgver 1.7.1dev.1, .SRCINFO source under /v1.7.1-dev.1/; spec Version 1.7.1~dev.1 and top changelog
#      "- 1.7.1~dev.1-1"; metainfo top release 1.7.1~dev.1 type development, then 1.7.0 stable.
#   B2 1.7.1-dev.2: exactly one ~dev changelog entry and one development release, both 1.7.1~dev.2.
#   B3 1.7.1 --note "Fixes.": top changelog 1.7.1-1 with "- Fixes.", the next one 1.7.0-1; no ~dev left; metainfo
#      1.7.1 stable then 1.7.0 stable.
#   B4 1.7.2-dev.1 --tag: annotated tag v1.7.2-dev.1 at HEAD, no version file left modified.
#   B5 "1.8" is refused and no file changes.
#   B6 rpm's own vercmp: 1.7.0 < 1.7.1~dev.1 < 1.7.1~dev.2 < 1.7.1.
set -uo pipefail
R=$(cd "$(dirname "$0")/.." && pwd)
if [ "${1:-}" = "--faults" ]; then
  rc=0
  for f in nodocs keepdev; do
    out=$(FAULT=$f "$0" 2>&1); if [ $? = 0 ]; then echo "FAULT $f NOT caught"; rc=1; else echo "fault $f caught: $(grep -m1 FAIL <<<"$out")"; fi
  done
  [ $rc = 0 ] && echo "PASS: 2 of 2 planted faults caught" || echo "FAIL: a planted fault was not caught"; exit $rc
fi
W=$(mktemp -d /srv/devops/build/clanky-bump-gate.XXXXXX)
trap 'rm -rf "$W"' EXIT
git clone -q --shared --no-checkout "$R" "$W/t" && git -C "$W/t" checkout -q --detach "$(git -C "$R" rev-parse HEAD)" || exit 1
cp "$R/scripts/bump-version.py" "$R/scripts/packaging-version.py" "$W/t/scripts/"
cd "$W/t"; git config user.name gate; git config user.email gate@example.invalid
case ${FAULT:-} in
  nodocs) sed -i "/sub1('docs\/agent-api.md'/d" scripts/bump-version.py ;;
  keepdev) sed -i "s/    log = re.sub(/    log = log or re.sub(/" scripts/bump-version.py ;;
esac
fail() { echo "FAIL: $*"; exit 1; }
B=scripts/bump-version.py; S=packaging/fedora/clanky.spec; M=packaging/linux/io.github.rustytensor.Clanky.metainfo.xml
top() { sed -n '/^%changelog/{n;p;q}' $S; }
rels() { grep -o '<release version="[^"]*" date="[^"]*" type="[a-z]*"' $M | head -${1:-2} | sed 's/ date="[^"]*"//'; }

python3 $B 1.7.1-dev.1 --date 2026-11-15 >/dev/null || fail "B1 bump failed"
grep -qx 'version = "1.7.1-dev.1"' Cargo.toml || fail "B1 Cargo.toml"
grep -A1 '^name = "clanky"$' Cargo.lock | grep -qx 'version = "1.7.1-dev.1"' || fail "B1 Cargo.lock"
grep -q '"version":"1.7.1-dev.1"' docs/agent-api.md || fail "B1 agent-api"
grep -qx 'pkgver=1.7.1dev.1' packaging/arch/PKGBUILD || fail "B1 PKGBUILD"
grep -q 'pkgver = 1.7.1dev.1$' packaging/arch/.SRCINFO && grep -q '/v1.7.1-dev.1/clanky-1.7.1dev.1.tar.gz' packaging/arch/.SRCINFO || fail "B1 .SRCINFO"
grep -qE '^Version:\s+1.7.1~dev.1$' $S && top | grep -q ' - 1.7.1~dev.1-1$' || fail "B1 spec: $(top)"
[ "$(rels)" = $'<release version="1.7.1~dev.1" type="development"\n<release version="1.7.0" type="stable"' ] || fail "B1 metainfo: $(rels)"
echo "ok B1"
python3 $B 1.7.1-dev.2 --date 2026-11-15 >/dev/null || fail "B2 bump failed"
[ "$(grep -c '~dev' <(sed -n '/^%changelog/,$p' $S))" = 1 ] && top | grep -q ' - 1.7.1~dev.2-1$' || fail "B2 spec dev entries: $(grep '~dev' $S)"
[ "$(grep -c 'type="development"' $M)" = 1 ] && rels 1 | grep -q '1.7.1~dev.2' || fail "B2 metainfo: $(rels 3)"
echo "ok B2"
python3 $B 1.7.1 --note "Fixes." --date 2026-11-16 >/dev/null || fail "B3 bump failed"
[ "$(sed -n '/^%changelog/,$p' $S | grep '^\*' | head -2 | sed 's/.* - //')" = $'1.7.1-1\n1.7.0-1' ] || fail "B3 changelog heads"
sed -n '/^%changelog/{n;n;p;q}' $S | grep -qx -- '- Fixes.' || fail "B3 note"
! grep -q '~dev' $S || fail "B3 dev entry left"
[ "$(rels)" = $'<release version="1.7.1" type="stable"\n<release version="1.7.0" type="stable"' ] || fail "B3 metainfo: $(rels)"
echo "ok B3"
python3 $B 1.7.2-dev.1 --tag >/dev/null || fail "B4 bump failed"
[ "$(git cat-file -t v1.7.2-dev.1 2>/dev/null)" = tag ] && [ "$(git rev-parse v1.7.2-dev.1^{commit})" = "$(git rev-parse HEAD)" ] || fail "B4 tag"
[ -z "$(git status --porcelain -- Cargo.toml Cargo.lock docs packaging)" ] || fail "B4 files left modified"
echo "ok B4"
before=$(git status --porcelain | md5sum)
python3 $B 1.8 >/dev/null 2>&1 && fail "B5 1.8 accepted"
[ "$(git status --porcelain | md5sum)" = "$before" ] || fail "B5 files changed"
echo "ok B5"
v() { rpm --eval "%{lua: print(rpm.vercmp('$1', '$2'))}"; }
[ "$(v 1.7.0 1.7.1~dev.1)$(v 1.7.1~dev.1 1.7.1~dev.2)$(v 1.7.1~dev.2 1.7.1)" = "-1-1-1" ] || fail "B6 rpm order"
echo "ok B6"
echo "PASS: bump-version, 6 of 6"
