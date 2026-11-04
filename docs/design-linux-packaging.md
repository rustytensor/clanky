# Linux packaging: design and acceptance gate

The package contains the current Clanky binary, desktop entry, AppStream metadata,
original SVG icon, and a roff man page. A source tarball contains Cargo's vendored
dependencies and a Cargo source-replacement configuration; the RPM build uses
`cargo --locked --offline`. No Rust source changes or downloads are part of this work.
The forked crates are declared as bundled at their actual versions: clanky-vt
0.26.0 and clanky-vte 0.15.0. The source and binary package licence is
`MIT OR Apache-2.0`.

## Gate

Run `scripts/gate-linux-packaging.sh`. It prints PASS only when all of these
independent expectations hold:

Before any build, the Cargo workspace version must equal the RPM spec Version,
the top RPM changelog version, the current AppStream release version and the
source tarball filename. With
`CLANKY_GATE_FAULT=version`, a temporary spec says 0.0.0 and this comparison
must fail before a build. The clean gate also builds an SRPM offline and checks
its RPM version and its source-tarball filename against the workspace version.

1. `desktop-file-validate` and `appstreamcli validate --no-net` succeed. Both
   the source desktop file and the desktop file extracted from the RPM contain
   exactly one each of `Exec=clanky`, `Icon=io.github.rustytensor.Clanky`, and
   `Name=Clanky`. Mismatches print the expected and observed lines.
2. `scripts/vendor-tarball.sh OUTPUT` produces a vendored source archive;
   `rpmbuild -bb` builds it with offline Cargo, at most eight jobs, under
   `/srv/devops/build/clanky-pkg`, with the shared build lock held. The build
   including lock wait has a 30-minute deadline; timeout is failure.
3. `rpm -qlp` gives exactly the following sorted paths, plus only RPM build-ID
   entries matching `^/usr/lib/\.build-id(/[0-9a-f]{2}(/[0-9a-f]{38}(\.debug)?)?)?$`.
   Build-ID directories must be directories; at least one build-ID link must
   exist, and every build-ID link must resolve to `/usr/bin/clanky` according
   to RPM's file metadata. No other extra paths are accepted:

```
/usr/bin/clanky
/usr/share/applications/clanky.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
/usr/share/licenses/clanky
/usr/share/licenses/clanky/LICENSE-APACHE
/usr/share/licenses/clanky/LICENSE-MIT
/usr/share/man/man1/clanky.1.gz
/usr/share/metainfo/io.github.rustytensor.Clanky.metainfo.xml
```

4. `rpmlint` reports no errors for the spec and binary RPM; warnings remain
   visible in the gate output and are listed in the result report.
5. `CLANKY_GATE_FAULT=desktop scripts/gate-linux-packaging.sh` changes only a
   temporary desktop entry's Exec to `Exec=clanky-broken`. The exact Exec check
   must print FAIL with expected and observed values before any build, even if
   desktop validation accepts it. Run this fault once, then the clean
   gate once; report both results. The temporary entry does not change the repo.

The source archive and RPM are written under the build directory, not the home
filesystem. The package's full RPM contents are checked against the list and
constrained build-ID rule above. Network-free Cargo is enforced by `--offline` and vendored
source replacement; this gate does not claim an OS-level network sandbox.
