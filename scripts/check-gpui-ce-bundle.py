#!/usr/bin/env python3
"""Check that third_party/gpui-ce is our copy of gpui-ce at the commit its SOURCE file names, file for file.

    scripts/check-gpui-ce-bundle.py [--root DIR] [--clone DIR]
    scripts/check-gpui-ce-bundle.py --faults

The expected files come from the copy's git objects (read with git, not from a checkout), the expected crates from
cargo's own resolution of the bundled workspace, and the expected list of changed files from the copy's history. The
script that wrote the bundle is not consulted. Exit 0 when everything matches; exit 1 with the first difference.

--clone DIR  a clone of the copy to read from; without it the check clones git.servorra.com/servorra/gpui-ce.
--faults     plant three faults in a scratch copy of the bundle (a bundled file edited, a crate left out, a git source
             on our server left in Cargo.lock) and require the check to catch each one.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

COPY_SSH_URL = "git@git.servorra.com:servorra/gpui-ce.git"
ROOT_PACKAGE = "clanky-gpui"
NOTICE_LINE = "Modified by the Clanky authors."
README_BEGIN = "<!-- bundle:begin -->"
README_END = "<!-- bundle:end -->"
SLASHES = {".rs", ".wgsl", ".glsl", ".c", ".h", ".js"}
HASHES = {".toml", ".sh", ".py", ".yml", ".yaml"}
SYMLINK_MODE = "120000"
DEFAULT_CARGO_HOME = "/srv/devops/cache/clanky-cargo"


class Difference(Exception):
    """The first way the bundle differs from what it should be."""


def run(
    args: list[str], cwd: Path | None = None, check: bool = True
) -> subprocess.CompletedProcess[bytes]:
    done = subprocess.run(args, cwd=cwd, capture_output=True, check=False)
    if check and done.returncode:
        raise Difference(
            f"{' '.join(args[:3])} failed: {done.stderr.decode(errors='replace').strip()}"
        )
    return done


def git(clone: Path, *args: str) -> bytes:
    return run(["git", "-C", str(clone), *args]).stdout


def read_source(bundle: Path) -> dict[str, str]:
    path = bundle / "SOURCE"
    if not path.is_file():
        raise Difference(f"{path} is missing")
    fields = dict(
        line.split("=", 1) for line in path.read_text().splitlines() if "=" in line
    )
    for key in ("repository", "rev", "upstream", "upstream_base"):
        if key not in fields:
            raise Difference(f"{path} has no {key}= line")
    if not re.fullmatch(r"[0-9a-f]{40}", fields["rev"]):
        raise Difference(f"{path}: rev {fields['rev']!r} is not a full commit hash")
    return fields


def tree_at(clone: Path, rev: str, folder: str) -> dict[str, tuple[str, str]]:
    """Every file under one crate folder at the commit: path -> (mode, object id)."""
    listing = git(clone, "ls-tree", "-r", "-z", rev, "--", folder).split(b"\0")
    files = {}
    for entry in filter(None, listing):
        meta, name = entry.split(b"\t", 1)
        mode, _kind, object_id = meta.decode().split()
        files[name.decode()] = (mode, object_id)
    return files


def with_notice(text: bytes, name: str) -> bytes | None:
    """The file's text with our marking line taken out, or None when the file does not carry it where it must."""
    suffix = Path(name).suffix
    if suffix in SLASHES:
        line = f"// {NOTICE_LINE}\n".encode()
    elif suffix in HASHES:
        line = f"# {NOTICE_LINE}\n".encode()
    elif suffix == ".md":
        line = f"<!-- {NOTICE_LINE} -->\n".encode()
    else:
        return text  # no comment syntax: the file is expected unmarked
    cut = text.index(b"\n") + 1 if text.startswith(b"#!") else 0
    if text[cut : cut + len(line)] != line:
        return None
    return text[:cut] + text[cut + len(line) :]


def check_lock_and_manifests(root: Path) -> None:
    lock = root / "Cargo.lock"
    for number, line in enumerate(lock.read_text().splitlines(), 1):
        if line.startswith("source = ") and "git+" in line and "servorra" in line:
            raise Difference(
                f"{lock}:{number} still names a git source on our server: {line}"
            )
    for manifest in [root / "Cargo.toml", *sorted(root.glob("crates/*/Cargo.toml"))]:
        for number, line in enumerate(manifest.read_text().splitlines(), 1):
            if (
                "servorra" in line
                and "git" in line
                and not line.lstrip().startswith("#")
            ):
                raise Difference(
                    f"{manifest}:{number} still depends on our server by git: {line.strip()}"
                )


def bundled_crates(root: Path, bundle: Path) -> dict[str, str]:
    """name -> folder of every gpui-ce crate in clanky-gpui's closure, as cargo resolves the bundled tree."""
    env = {**os.environ, "CARGO_HOME": os.environ.get("CARGO_HOME", DEFAULT_CARGO_HOME)}
    done = subprocess.run(
        ["cargo", "metadata", "--offline", "--format-version", "1"],
        cwd=root,
        capture_output=True,
        env=env,
        check=False,
    )
    if done.returncode:
        lines = [
            x.strip()
            for x in done.stderr.decode(errors="replace").splitlines()
            if x.strip()
        ]
        first = next((i for i, x in enumerate(lines) if x.startswith("error:")), 0)
        reasons = " / ".join(x for x in lines[first:] if x != "Caused by:")
        raise Difference(
            f"cargo cannot resolve the workspace (a crate left out of the bundle fails here): {reasons}"
        )
    meta = json.loads(done.stdout)
    by_id = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    start = next((i for i, p in by_id.items() if p["name"] == ROOT_PACKAGE), None)
    if start is None:
        raise Difference(f"no package {ROOT_PACKAGE} in the workspace")
    seen: set[str] = set()
    stack = [start]
    while stack:
        package_id = stack.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        stack += [
            d["pkg"]
            for d in nodes[package_id]["deps"]
            if any(k["kind"] in (None, "build") for k in d["dep_kinds"])
        ]
    found = {}
    for package_id in seen:
        manifest = Path(by_id[package_id]["manifest_path"])
        if manifest.is_relative_to(bundle):
            found[by_id[package_id]["name"]] = manifest.parent.name
        elif "servorra/gpui-ce" in (by_id[package_id]["source"] or ""):
            raise Difference(
                f"{by_id[package_id]['name']} still comes from our server, not from the bundle"
            )
    return found


def modified_files(
    clone: Path, upstream_base: str, rev: str, folders: list[str]
) -> list[str]:
    out = []
    for folder in folders:
        out += (
            git(
                clone,
                "diff",
                "--name-only",
                "--diff-filter=M",
                upstream_base,
                rev,
                "--",
                f"crates/{folder}",
            )
            .decode()
            .split()
        )
    return sorted(out)


def added_files(
    clone: Path, upstream_base: str, rev: str, folders: list[str]
) -> list[str]:
    out = []
    for folder in folders:
        out += (
            git(
                clone,
                "diff",
                "--name-only",
                "--diff-filter=A",
                upstream_base,
                rev,
                "--",
                f"crates/{folder}",
            )
            .decode()
            .split()
        )
    return sorted(out)


def check_crate_files(
    bundle: Path, clone: Path, rev: str, folders: list[str], modified: set[str]
) -> tuple[int, int]:
    count = size = 0
    for folder in folders:
        expected = tree_at(clone, rev, f"crates/{folder}")
        on_disk = {
            str(p.relative_to(bundle)): p
            for p in sorted((bundle / "crates" / folder).rglob("*"))
            if not p.is_dir() or p.is_symlink()
        }
        for name in sorted(set(on_disk) - set(expected)):
            raise Difference(f"{bundle / name} is not in the copy at {rev[:10]}")
        for name in sorted(set(expected) - set(on_disk)):
            raise Difference(
                f"{bundle / name} is missing; the copy has it at {rev[:10]}"
            )
        for name, (mode, object_id) in expected.items():
            path = on_disk[name]
            want = git(clone, "cat-file", "blob", object_id)
            if mode == SYMLINK_MODE:
                same = path.is_symlink() and os.readlink(path).encode() == want
            else:
                got: bytes | None = path.read_bytes()
                if name in modified and got is not None:
                    got = with_notice(got, name)
                    if got is None:
                        raise Difference(
                            f'{path} was changed by Clanky but lacks the line "{NOTICE_LINE}"'
                        )
                same = got == want
            if not same:
                raise Difference(f"{path} differs from the copy at {rev[:10]}")
            count += 1
            size += path.lstat().st_size
    return count, size


def check_workspace_manifest(
    bundle: Path, clone: Path, rev: str, folders: list[str]
) -> None:
    mine = tomllib.loads((bundle / "Cargo.toml").read_text())
    theirs = tomllib.loads(git(clone, "show", f"{rev}:Cargo.toml").decode())
    want = [f"./crates/{f}/" for f in folders]
    if sorted(mine["workspace"]["members"]) != sorted(want):
        raise Difference(
            f"{bundle / 'Cargo.toml'}: members are {mine['workspace']['members']}, expected {sorted(want)}"
        )
    for section in ("package", "dependencies", "lints"):
        if mine["workspace"].get(section) != theirs["workspace"].get(section):
            raise Difference(
                f"{bundle / 'Cargo.toml'}: [workspace.{section}] differs from the copy's"
            )


def check_licences(bundle: Path, clone: Path, rev: str) -> None:
    licence = bundle / "LICENSE.md"
    if not licence.is_file():
        raise Difference(f"{licence} is missing")
    if licence.read_bytes() != git(clone, "show", f"{rev}:LICENSE.md"):
        raise Difference(f"{licence} differs from the copy's")
    notices = bundle / "NOTICES.md"
    if not notices.is_file():
        raise Difference(f"{notices} is missing")
    text = notices.read_text()
    for needle in (
        "figma-squircle",
        "Tien Pham",
        "Microsoft Corporation",
        "Permission is hereby granted",
    ):
        if needle not in text:
            raise Difference(f"{notices} lacks the notice text {needle!r}")


def check_readme(root: Path, modified: list[str], added: list[str]) -> None:
    readme = root / "third_party" / "README.md"
    text = readme.read_text()
    if README_BEGIN not in text or README_END not in text:
        raise Difference(
            f"{readme} has no filled list between {README_BEGIN} and {README_END}"
        )
    listed = set(
        re.findall(
            r"^- `third_party/gpui-ce/([^`]+)`",
            text.split(README_BEGIN)[1].split(README_END)[0],
            re.M,
        )
    )
    if listed != set(modified) | set(added):
        raise Difference(
            f"{readme} lists {sorted(listed)}, the copy's history says {sorted(set(modified) | set(added))}"
        )


def check(root: Path, clone: Path) -> str:
    bundle = root / "third_party" / "gpui-ce"
    check_lock_and_manifests(root)
    source = read_source(bundle)
    rev = source["rev"]
    crates = bundled_crates(root, bundle)
    folders = sorted(crates.values())
    on_disk = sorted(p.name for p in (bundle / "crates").iterdir() if p.is_dir())
    if on_disk != folders:
        missing, extra = (
            sorted(set(folders) - set(on_disk)),
            sorted(set(on_disk) - set(folders)),
        )
        raise Difference(
            f"crates in {bundle / 'crates'}: missing {missing}, extra {extra}"
        )
    check_licences(bundle, clone, rev)
    check_workspace_manifest(bundle, clone, rev, folders)
    modified = modified_files(clone, source["upstream_base"], rev, folders)
    added = added_files(clone, source["upstream_base"], rev, folders)
    count, size = check_crate_files(bundle, clone, rev, folders, set(modified))
    check_readme(root, modified, added)
    return (
        f"PASS: {len(folders)} crates, {count} files, {size / 1e6:.1f} MB equal the copy at {rev[:10]}; "
        f"{len(modified)} marked"
    )


def plant_faults(root: Path, clone: Path) -> int:
    """Each fault in a scratch copy of what the check reads; the check must fail on it with the expected words."""
    faults = [
        ("a bundled file edited by hand", "differs from the copy", edit_one_file),
        ("a crate left out", "gpui_refineable", leave_one_crate_out),
        (
            "a git source on our server in Cargo.lock",
            "still names a git source",
            add_git_source,
        ),
    ]
    failures = 0
    for label, expected, plant in faults:
        with tempfile.TemporaryDirectory(
            dir="/srv/devops/build", prefix="check-bundle-fault."
        ) as scratch:
            copy = Path(scratch) / "tree"
            copy.mkdir()
            for item in ("Cargo.toml", "Cargo.lock"):
                shutil.copy2(root / item, copy / item)
            for folder in ("crates", "third_party"):
                shutil.copytree(root / folder, copy / folder, symlinks=True)
            plant(copy)
            try:
                check(copy, clone)
                print(f"FAULT NOT CAUGHT: {label}")
                failures += 1
            except Difference as difference:
                if expected in str(difference):
                    print(f"CAUGHT: {label}: {difference}")
                else:
                    print(f"WRONG REASON: {label}: {difference}")
                    failures += 1
    return failures


def edit_one_file(root: Path) -> None:
    target = sorted(
        (root / "third_party/gpui-ce/crates/gpui_sum_tree/src").glob("*.rs")
    )[0]
    target.write_text(target.read_text() + "\n// edited by hand\n")


def leave_one_crate_out(root: Path) -> None:
    shutil.rmtree(root / "third_party/gpui-ce/crates/gpui_refineable")


def add_git_source(root: Path) -> None:
    lock = root / "Cargo.lock"
    lock.write_text(
        lock.read_text().replace(
            'name = "gpui_ce_refineable"\nversion = "0.4.0"\n',
            'name = "gpui_ce_refineable"\nversion = "0.4.0"\n'
            'source = "git+https://git.servorra.com/servorra/gpui-ce?rev=b327731fe9b8afa857d67bd68a24e4127ef45d29#b327731fe9b8afa857d67bd68a24e4127ef45d29"\n',
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    parser.add_argument("--clone", type=Path)
    parser.add_argument("--faults", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    scratch = None
    clone = args.clone
    try:
        if clone is None:
            scratch = tempfile.mkdtemp(
                dir="/srv/devops/build", prefix="check-gpui-ce-bundle."
            )
            clone = Path(scratch) / "copy"
            run(["git", "clone", "-q", COPY_SSH_URL, str(clone)])
        if args.faults:
            failures = plant_faults(root, clone)
            print(
                "faults: all caught"
                if not failures
                else f"faults: {failures} not caught"
            )
            return 1 if failures else 0
        print(check(root, clone))
        return 0
    except Difference as difference:
        print(f"FAIL: {difference}")
        return 1
    finally:
        if scratch:
            shutil.rmtree(scratch, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
