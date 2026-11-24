#!/usr/bin/env python3
"""Set Clanky's version in every file that holds it, so the tag and the files agree.

Usage: scripts/bump-version.py VERSION [--note TEXT]... [--date YYYY-MM-DD] [--tag [--message TEXT]]

VERSION is SemVer: 1.7.1 (a release), 1.7.1-rc.1, or 1.8.0-dev.3 (a commit before the release it leads to; every
commit gets its own tag, so N rises by one per commit and a number is never used twice). Files: Cargo.toml and Cargo.lock, docs/agent-api.md, packaging/arch/PKGBUILD and .SRCINFO,
packaging/fedora/clanky.spec (Version and %changelog), the AppStream metainfo (releases).
- A release or rc adds a %changelog entry (one line per --note; at least one is required) and a <release> line.
- A dev version adds one "Development build" entry and a type="development" <release>; the next dev version replaces
  them, and a release replaces the dev ones, so the changelog lists only real versions.
- Packages write "-" as "~" (RPM and AppStream sort 1.7.2~dev.3 before 1.7.2); Arch drops it (1.7.2dev.3 sorts
  before 1.7.2 in vercmp, which ranks letters below the end of a version).
--tag commits the version files together with whatever else is staged, so the bump is part of the commit it tags,
and makes the annotated tag vVERSION on it. --message is that commit's message (default: "Version VERSION").
Ends by running scripts/packaging-version.py --check.
"""

import argparse
import datetime
import pathlib
import re
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
FORM = r"[0-9]+\.[0-9]+\.[0-9]+(-(rc|dev)\.[0-9]+)?"
SPEC = ROOT / "packaging/fedora/clanky.spec"
META = ROOT / "packaging/linux/io.github.rustytensor.Clanky.metainfo.xml"


def sub1(path, pattern, repl, flags=re.MULTILINE):
    p = ROOT / path
    s, n = re.subn(pattern, repl, p.read_text(), flags=flags)
    if n == 0:
        sys.exit(f"FAIL: {path}: nothing matched {pattern!r}")
    p.write_text(s)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("version")
    ap.add_argument("--note", action="append", default=[])
    ap.add_argument("--date", default=datetime.date.today().isoformat())
    ap.add_argument("--tag", action="store_true")
    ap.add_argument("--message")
    a = ap.parse_args()
    new = a.version.removeprefix("v")
    if not re.fullmatch(FORM, new):
        sys.exit(f"FAIL: {new!r} is not 1.2.3, 1.2.3-rc.N or 1.2.3-dev.N")
    if a.message and not a.tag:
        sys.exit("FAIL: --message is the commit message of --tag; give both or neither")
    dev = "-dev." in new
    if not dev and not a.note:
        sys.exit("FAIL: a release or rc needs at least one --note for the changelog")
    old = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"][
        "version"
    ]
    pkg_old, pkg_new = old.replace("-", "~"), new.replace("-", "~")
    arch_old, arch_new = old.replace("-", ""), new.replace("-", "")
    date = datetime.date.fromisoformat(a.date)

    sub1("Cargo.toml", rf'^version = "{re.escape(old)}"', f'version = "{new}"')
    sub1("docs/agent-api.md", rf'"version":"{re.escape(old)}"', f'"version":"{new}"')
    sub1(
        "packaging/arch/PKGBUILD",
        rf"^pkgver={re.escape(arch_old)}$",
        f"pkgver={arch_new}",
    )
    srcinfo = ROOT / "packaging/arch/.SRCINFO"
    s = (
        srcinfo.read_text()
        .replace(f"/v{old}/", f"/v{new}/")
        .replace(f"clanky-{arch_old}", f"clanky-{arch_new}")
    )
    srcinfo.write_text(
        re.sub(
            rf"pkgver = {re.escape(arch_old)}$",
            f"pkgver = {arch_new}",
            s,
            flags=re.MULTILINE,
        )
    )
    sub1(
        "packaging/fedora/clanky.spec",
        rf"^(Version:\s*){re.escape(pkg_old)}$",
        rf"\g<1>{pkg_new}",
    )

    # %changelog: drop a top dev entry, then add the new one.
    spec = SPEC.read_text()
    head, log = spec.split("%changelog\n", 1)
    log = re.sub(
        r"\A\* [^\n]* - [0-9.]+~dev\.[0-9]+-[^\n]*\n(?:- [^\n]*\n)*\n?", "", log
    )
    notes = a.note or ["Development build."]
    entry = (
        f"* {date.strftime('%a %b %d %Y')} Clanky contributors <noreply@github.com> - {pkg_new}-1\n"
        + "".join(f"- {n}\n" for n in notes)
        + "\n"
    )
    SPEC.write_text(head + "%changelog\n" + entry + log)

    # AppStream releases: drop development lines, then add the new one on top.
    m = META.read_text()
    m = re.sub(
        r'    <release version="[^"]*" date="[^"]*" type="development"/>\n', "", m
    )
    kind = "development" if dev or "-rc." in new else "stable"
    m = m.replace(
        "  <releases>\n",
        f'  <releases>\n    <release version="{pkg_new}" date="{date}" type="{kind}"/>\n',
        1,
    )
    META.write_text(m)

    subprocess.run(["cargo", "update", "-w", "--offline", "-q"], cwd=ROOT, check=True)
    got = subprocess.run(
        [sys.executable, str(ROOT / "scripts/packaging-version.py"), "--check"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if got.returncode or got.stdout.strip() != pkg_new:
        sys.exit(
            f"FAIL: packaging-version.py --check: {got.stdout.strip()} {got.stderr.strip()}"
        )
    files = [
        "Cargo.toml",
        "Cargo.lock",
        "docs/agent-api.md",
        "packaging/arch/PKGBUILD",
        "packaging/arch/.SRCINFO",
        "packaging/fedora/clanky.spec",
        str(META.relative_to(ROOT)),
    ]
    if a.tag:
        subprocess.run(["git", "add", *files], cwd=ROOT, check=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", a.message or f"Version {new}"],
            cwd=ROOT,
            check=True,
        )
        subprocess.run(
            ["git", "tag", "-a", f"v{new}", "-m", f"v{new}"], cwd=ROOT, check=True
        )
    print(f"{old} -> {new}" + (f" (tag v{new})" if a.tag else ""))


if __name__ == "__main__":
    main()
