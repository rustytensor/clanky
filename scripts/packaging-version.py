#!/usr/bin/env python3
"""Read the workspace version and optionally check packaging inputs."""
import argparse
import pathlib
import re
import sys
import tomllib
import xml.etree.ElementTree as ET

root = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--check', action='store_true')
parser.add_argument('--spec', type=pathlib.Path, default=root / 'packaging/fedora/clanky.spec')
parser.add_argument('--tarball', type=pathlib.Path)
args = parser.parse_args()
with (root / 'Cargo.toml').open('rb') as manifest:
    version = tomllib.load(manifest)['workspace']['package']['version']
if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?', version):
    sys.exit(f'FAIL: unsupported workspace version {version!r}')
# Packages write a release candidate with a tilde (1.0.0-rc.1 -> 1.0.0~rc.1) so RPM and AppStream sort it before 1.0.0.
version = version.replace('-', '~')
if args.check:
    spec = args.spec.read_text()
    versions = re.findall(r'^Version:\s*(\S+)\s*$', spec, re.MULTILINE)
    sections = re.split(r'^%changelog\s*$', spec, flags=re.MULTILINE)
    header = re.search(r'^\*.*$', sections[1], re.MULTILINE) if len(sections) == 2 else None
    entry = re.fullmatch(r'\* .+ - ([0-9]+\.[0-9]+\.[0-9]+(?:~rc\.[0-9]+)?)-\S+', header.group()) if header else None
    release = ET.parse(root / 'packaging/linux/io.github.rustytensor.Clanky.metainfo.xml').find('./releases/release')
    observed = {'spec Version': versions, 'spec top changelog version': [entry.group(1)] if entry else [], 'AppStream current release': [] if release is None else [release.get('version')]}
    if args.tarball is not None:
        observed['tarball name'] = [args.tarball.name]
    for name, actual in observed.items():
        expected = f'clanky-{version}.tar.gz' if name == 'tarball name' else version
        if actual != [expected]:
            sys.exit(f'FAIL: {name}: expected {expected!r}; observed {actual!r}')
print(version)
