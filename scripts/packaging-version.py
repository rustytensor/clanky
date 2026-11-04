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
if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', version):
    sys.exit(f'FAIL: unsupported workspace version {version!r}')
if args.check:
    versions = re.findall(r'^Version:\s*(\S+)\s*$', args.spec.read_text(), re.MULTILINE)
    release = ET.parse(root / 'packaging/linux/io.github.rustytensor.Clanky.metainfo.xml').find('./releases/release')
    observed = {'spec Version': versions, 'AppStream current release': [] if release is None else [release.get('version')]}
    if args.tarball is not None:
        observed['tarball name'] = [args.tarball.name]
    for name, actual in observed.items():
        expected = f'clanky-{version}.tar.gz' if name == 'tarball name' else version
        if actual != [expected]:
            sys.exit(f'FAIL: {name}: expected {expected!r}; observed {actual!r}')
print(version)
