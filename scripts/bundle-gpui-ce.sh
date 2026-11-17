#!/usr/bin/env bash
# Bundle the gpui-ce crates Clanky builds into third_party/gpui-ce, from our copy of gpui-ce at one commit.
#
#   scripts/bundle-gpui-ce.sh <rev> [--clone DIR] [--base REV]
#
# <rev>       the commit of our copy to bundle (a full or short hash, or a ref the copy knows).
# --clone DIR a clean clone of the copy to read from; without it the script clones the copy into a scratch folder
#             and removes it when it is done.
# --base REV  the upstream commit our changes sit on, to find the files we changed; default: the merge base of <rev>
#             and the copy's main branch, which carries upstream unchanged.
#
# Which crates are copied is read from cargo, never listed here: the gpui-ce crates in the dependency closure of
# clanky-gpui, on every platform (cargo needs the manifests of the macOS and Windows crates to resolve the lock file
# even when it builds on Linux). The script owns third_party/gpui-ce and rewrites it whole on every run, so a moved pin
# is one run. scripts/check-gpui-ce-bundle.py proves the result matches the copy.
set -euo pipefail

readonly COPY_SSH_URL="git@git.servorra.com:servorra/gpui-ce.git"
readonly COPY_URL="https://git.servorra.com/servorra/gpui-ce"
readonly UPSTREAM_URL="https://github.com/gpui-ce/gpui-ce"
readonly ROOT_PACKAGE="clanky-gpui"
readonly NOTICE_LINE="Modified by the Clanky authors."

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_HOME="${CARGO_HOME:-/srv/devops/cache/clanky-cargo}"
bundle_dir="${repo_root}/third_party/gpui-ce"

die() {
  echo "bundle-gpui-ce: $*" >&2
  exit 1
}

usage() {
  sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

rev=""
clone=""
base=""
while (($#)); do
  case "$1" in
    --clone) clone="${2:?--clone needs a folder}"; shift 2 ;;
    --base) base="${2:?--base needs a commit}"; shift 2 ;;
    -h | --help) usage ;;
    -*) die "unknown option $1" ;;
    *) [[ -z "${rev}" ]] || die "one rev only"; rev="$1"; shift ;;
  esac
done
[[ -n "${rev}" ]] || usage

scratch="$(mktemp -d /srv/devops/build/bundle-gpui-ce.XXXXXX)"
trap 'rm -rf "${scratch}"' EXIT

if [[ -z "${clone}" ]]; then
  clone="${scratch}/copy"
  git clone -q "${COPY_SSH_URL}" "${clone}" ||
    die "cannot clone ${COPY_SSH_URL}: check the key and the network, or pass --clone with a clean clone"
fi
[[ -z "$(git -C "${clone}" status --porcelain)" ]] || die "${clone} is not clean; use a fresh clone"
sha="$(git -C "${clone}" rev-parse --verify "${rev}^{commit}")" ||
  die "${rev} is not a commit in ${clone}"
git -C "${clone}" checkout -q --detach "${sha}"
if [[ -z "${base}" ]]; then
  base="$(git -C "${clone}" merge-base "${sha}" origin/main)" ||
    die "no merge base of ${sha} and origin/main in ${clone}; pass --base"
fi
base="$(git -C "${clone}" rev-parse --verify "${base}^{commit}")"

# The crates: name and folder under crates/, one per line, from cargo's own resolution of a probe that depends on
# the same gpui-ce crates, with the same features, as ROOT_PACKAGE does.
closure() {
  local probe="${scratch}/probe"
  mkdir -p "${probe}/src"
  : >"${probe}/src/lib.rs"
  cp "${repo_root}/Cargo.lock" "${probe}/Cargo.lock"
  python3 -I - "${repo_root}" "${clone}" "${probe}" "${ROOT_PACKAGE}" <<'PY'
import json, subprocess, sys, tomllib
from pathlib import Path

repo, clone, probe, root_package = sys.argv[1:5]
clone_crates = Path(clone) / "crates"


def cargo_metadata(manifest, *flags):
    done = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", manifest, *flags],
        capture_output=True, text=True, check=False)
    if done.returncode:
        sys.exit(f"cargo metadata on {manifest} failed: {done.stderr.strip()}")
    return json.loads(done.stdout)


def is_gpui_ce(dep):
    # Before bundling the crates come from our copy by git; after, by path under third_party.
    source = dep.get("source") or ""
    path = dep.get("path") or ""
    return "servorra/gpui-ce" in source or "third_party/gpui-ce" in path


workspace = cargo_metadata(f"{repo}/Cargo.toml", "--no-deps")
package = next((p for p in workspace["packages"] if p["name"] == root_package), None)
if package is None:
    sys.exit(f"no package {root_package} in {repo}/Cargo.toml")
folder_of = {}
for manifest in clone_crates.glob("*/Cargo.toml"):
    folder_of[tomllib.loads(manifest.read_text())["package"]["name"]] = manifest.parent.name
lines = ["[workspace]", "", "[package]", 'name = "bundle-probe"', 'version = "0.0.0"',
         'edition = "2024"', "", "[dependencies]"]
for dep in package["dependencies"]:
    if not is_gpui_ce(dep) or dep["kind"] == "dev":
        continue
    if dep["name"] not in folder_of:
        sys.exit(f"{root_package} needs {dep['name']}, which is not in {clone_crates}")
    key = dep.get("rename") or dep["name"]
    features = ", ".join(f'"{f}"' for f in dep["features"])
    default = "" if dep["uses_default_features"] else ", default-features = false"
    lines.append(f'{key} = {{ package = "{dep["name"]}", path = "{clone_crates / folder_of[dep["name"]]}", '
                 f'features = [{features}]{default} }}')
Path(probe, "Cargo.toml").write_text("\n".join(lines) + "\n")

meta = cargo_metadata(f"{probe}/Cargo.toml", "--offline")
by_id = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
seen, stack = set(), [meta["resolve"]["root"]]
while stack:
    pkg_id = stack.pop()
    if pkg_id in seen:
        continue
    seen.add(pkg_id)
    for dep in nodes[pkg_id]["deps"]:
        if any(k["kind"] in (None, "build") for k in dep["dep_kinds"]):
            stack.append(dep["pkg"])
found = sorted(
    (by_id[i]["name"], Path(by_id[i]["manifest_path"]).parent.name)
    for i in seen
    if Path(by_id[i]["manifest_path"]).is_relative_to(clone_crates))
for name, folder in found:
    print(f"{name}\t{folder}")
PY
}

crates_file="${scratch}/crates.tsv"
closure >"${crates_file}"
[[ -s "${crates_file}" ]] || die "cargo found no gpui-ce crate in the closure of ${ROOT_PACKAGE}"

# The script owns third_party/gpui-ce.
rm -rf "${bundle_dir}"
mkdir -p "${bundle_dir}/crates"
while IFS=$'\t' read -r _ folder; do
  cp -a "${clone}/crates/${folder}" "${bundle_dir}/crates/${folder}"
done <"${crates_file}"
cp "${clone}/LICENSE.md" "${bundle_dir}/LICENSE.md"

# Files our copy changed against upstream, among the bundled crates: Apache-2.0 section 4(b) wants each marked.
modified_file="${scratch}/modified.txt"
added_file="${scratch}/added.txt"
: >"${modified_file}"
: >"${added_file}"
while IFS=$'\t' read -r _ folder; do
  git -C "${clone}" diff --name-only --diff-filter=M "${base}" "${sha}" -- "crates/${folder}" >>"${modified_file}"
  git -C "${clone}" diff --name-only --diff-filter=A "${base}" "${sha}" -- "crates/${folder}" >>"${added_file}"
done <"${crates_file}"

python3 -I - "${bundle_dir}" "${NOTICE_LINE}" "${modified_file}" "${repo_root}/third_party/README.md" \
  "${added_file}" <<'PY'
import sys
from pathlib import Path

bundle, notice, modified_list, readme_path, added_list = sys.argv[1:6]
SLASHES = {".rs", ".wgsl", ".glsl", ".c", ".h", ".js"}
HASHES = {".toml", ".sh", ".py", ".yml", ".yaml"}
no_comment = []
modified = sorted(Path(modified_list).read_text().split())
for name in modified:
    path = Path(bundle, name)
    suffix = path.suffix
    if suffix in SLASHES:
        line = f"// {notice}\n"
    elif suffix in HASHES:
        line = f"# {notice}\n"
    elif suffix == ".md":
        line = f"<!-- {notice} -->\n"
    else:
        no_comment.append(name)
        continue
    text = path.read_text()
    # After a shebang line, which must stay first.
    cut = text.index("\n") + 1 if text.startswith("#!") else 0
    path.write_text(text[:cut] + line + text[cut:])

marker_begin, marker_end = "<!-- bundle:begin -->", "<!-- bundle:end -->"
readme = Path(readme_path)
text = readme.read_text()
if marker_begin not in text or marker_end not in text:
    sys.exit(f"{readme} has no {marker_begin} ... {marker_end} region to fill")
body = ["", "Files changed by Clanky (each carries the line \"" + notice + "\" where its format allows a comment):", ""]
body += [f"- `third_party/gpui-ce/{name}`" + (" (no comment syntax: not marked in the file)" if name in no_comment else "")
         for name in modified]
added = sorted(Path(added_list).read_text().split())
if added:
    body += ["", "Files Clanky added to these crates:", ""]
    body += [f"- `third_party/gpui-ce/{name}`" for name in added]
head, rest = text.split(marker_begin, 1)
_, tail = rest.split(marker_end, 1)
readme.write_text(head + marker_begin + "\n".join(body) + "\n" + marker_end + tail)
PY

# Every crate's own notice files, and the one MIT notice that lives only in a source line, in one file.
python3 -I - "${bundle_dir}" "${clone}" <<'PY'
import sys
from pathlib import Path

bundle, clone = (Path(a) for a in sys.argv[1:3])
terminal_line = "// Copyright (c) Microsoft Corporation."
platform_rs = clone / "crates/gpui/src/platform.rs"
if terminal_line not in platform_rs.read_text():
    sys.exit(f"{platform_rs} no longer has the Microsoft Terminal copyright line: update this script's notice text")
parts = ["# Notices for code inside the bundled gpui-ce crates\n",
         "These are the notices the licences of code inside gpui-ce ask us to keep. The licence of gpui-ce itself is "
         "`LICENSE.md`. Each crate's own `LICENSE-APACHE` and notice files sit in its folder.\n"]
for notice in sorted(bundle.glob("crates/*/THIRD_PARTY_NOTICES*")):
    parts.append(f"## From `{notice.relative_to(bundle)}`\n\n{notice.read_text().strip()}\n")
parts.append(
    "## Microsoft Terminal\n\n"
    "The text contrast and gamma code in `crates/gpui/src/platform.rs` is adapted from "
    "<https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.cpp>.\n\n"
    "Copyright (c) Microsoft Corporation.\n\n"
    "MIT License\n\n"
    "Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated "
    "documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation "
    "the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and "
    "to permit persons to whom the Software is furnished to do so, subject to the following conditions:\n\n"
    "The above copyright notice and this permission notice shall be included in all copies or substantial portions of "
    "the Software.\n\n"
    "THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO "
    "THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE "
    "AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF "
    "CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER "
    "DEALINGS IN THE SOFTWARE.\n")
(bundle / "NOTICES.md").write_text("\n".join(parts))
PY

# The workspace manifest: upstream's, with the members cut down to the bundled crates, so the crates' own
# `workspace = true` references (versions, lints, edition) resolve inside third_party/gpui-ce.
python3 -I - "${clone}/Cargo.toml" "${bundle_dir}/Cargo.toml" "${crates_file}" <<'PY'
import re, sys
from pathlib import Path

source, target, crates_tsv = sys.argv[1:4]
folders = [line.split("\t")[1] for line in Path(crates_tsv).read_text().splitlines()]
text = Path(source).read_text()
members = "members = [\n" + "".join(f'  "./crates/{folder}/",\n' for folder in folders) + "]"
text, count = re.subn(r"^members = \[.*?^\]", lambda _: members, text, count=1, flags=re.S | re.M)
if count != 1:
    sys.exit(f"{source} has no members list to rewrite")
header = ("# gpui-ce's workspace manifest with the members cut down to the crates Clanky builds.\n"
          "# Written by scripts/bundle-gpui-ce.sh; do not edit by hand.\n")
Path(target).write_text(header + text)
PY

printf 'repository=%s\nrev=%s\nupstream=%s\nupstream_base=%s\n' \
  "${COPY_URL}" "${sha}" "${UPSTREAM_URL}" "${base}" >"${bundle_dir}/SOURCE"

# Clanky's own manifests: every dependency on gpui-ce becomes a path into the bundle.
python3 -I - "${repo_root}" "${crates_file}" <<'PY'
import os, re, sys
from pathlib import Path

repo = Path(sys.argv[1])
folder_of = dict(line.split("\t") for line in Path(sys.argv[2]).read_text().splitlines())
bundle_crates = repo / "third_party/gpui-ce/crates"
GIT_KEY = re.compile(r'\s*git\s*=\s*"[^"]*servorra/gpui-ce[^"]*"\s*,?')
REV_KEY = re.compile(r'\s*rev\s*=\s*"[0-9a-f]+"\s*,?')
LINE = re.compile(r'^(\w+\s*=\s*\{\s*package\s*=\s*"(gpui[-_]ce[\w-]*)"\s*,)(.*)(\}\s*)$')
OLD_COMMENT = "# Our copy of gpui-ce, pinned; never github.com (the copy exists so Clanky builds if upstream goes away).\n"
NEW_COMMENT = "# gpui-ce, bundled in third_party/gpui-ce at the commit its SOURCE file names; never fetched from the network.\n"
for manifest in [repo / "Cargo.toml", *sorted(repo.glob("crates/*/Cargo.toml"))]:
    out, changed = [], False
    for line in manifest.read_text().splitlines(keepends=True):
        match = LINE.match(line.rstrip("\n"))
        if match and "servorra/gpui-ce" in line:
            head, name, rest, tail = match.groups()
            if name not in folder_of:
                sys.exit(f"{manifest} depends on {name}, which is not in the bundle's crate list")
            rest = REV_KEY.sub("", GIT_KEY.sub("", rest)).strip().rstrip(",").strip()
            path = os.path.relpath(bundle_crates / folder_of[name], manifest.parent)
            line = f'{head} path = "{path}"' + (f", {rest}" if rest else "") + f" {tail}\n"
            changed = True
        elif line == OLD_COMMENT:
            line, changed = NEW_COMMENT, True
        out.append(line)
    if changed:
        manifest.write_text("".join(out))

# Path dependencies inside the workspace folder become workspace members, and gpui-ce's crates would then inherit
# from Clanky's workspace. Excluding the folder keeps them in their own workspace, whose manifest they expect.
root_manifest = repo / "Cargo.toml"
text = root_manifest.read_text()
if "exclude" not in text:
    text, count = re.subn(r"^(members = \[.*\]\n)", r'\1exclude = ["third_party/gpui-ce"]\n', text, count=1, flags=re.M)
    if count != 1:
        sys.exit(f"{root_manifest} has no one-line members list to put the exclude after")
    root_manifest.write_text(text)
PY

# Rewrite Cargo.lock for the path crates (offline: only the gpui-ce entries change), then make sure nothing points
# at our server any more.
(cd "${repo_root}" && cargo metadata --offline --format-version 1 >/dev/null) ||
  die "cargo cannot resolve the workspace with the bundled crates: see its message above"
if grep -q 'git+https://git.servorra.com' "${repo_root}/Cargo.lock"; then
  die "Cargo.lock still names a git source on git.servorra.com"
fi

crate_count="$(wc -l <"${crates_file}")"
file_count="$(find "${bundle_dir}" -type f | wc -l)"
size="$(du -sh "${bundle_dir}" | cut -f1)"
echo "bundled ${crate_count} crates from ${COPY_URL} at ${sha}: ${file_count} files, ${size}"
