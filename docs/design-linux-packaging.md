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

1. `desktop-file-validate` and `appstreamcli validate --no-net` succeed.
2. `scripts/vendor-tarball.sh OUTPUT` produces a vendored source archive;
   `rpmbuild -bb` builds it with offline Cargo, at most eight jobs, under
   `/srv/devops/build/clanky-pkg`, with the shared build lock held.
3. `rpm -qlp` gives exactly the following sorted paths (no directory entries):

```
/usr/bin/clanky
/usr/share/applications/clanky.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
/usr/share/licenses/clanky/LICENSE-APACHE
/usr/share/licenses/clanky/LICENSE-MIT
/usr/share/man/man1/clanky.1.gz
/usr/share/metainfo/io.github.rustytensor.Clanky.metainfo.xml
```

4. `rpmlint` reports no errors for the spec and binary RPM; warnings remain
   visible in the gate output and are listed in the result report.
5. `CLANKY_GATE_FAULT=desktop scripts/gate-linux-packaging.sh` changes only a
   temporary desktop entry's Exec to an invalid value. Validation must fail and
   the gate must print FAIL before any build. Run this fault once, then the clean
   gate once; report both results. The temporary entry does not change the repo.

The source archive and RPM are written under the build directory, not the home
filesystem. The package's full RPM contents, not a filtered subset, are compared
with the list above. Network-free Cargo is enforced by `--offline` and vendored
source replacement; this gate does not claim an OS-level network sandbox.
