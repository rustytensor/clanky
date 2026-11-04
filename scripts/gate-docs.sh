#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
trap 'rc=$?; if (( rc != 0 )); then echo "FAIL: documentation" >&2; fi' EXIT
python3 - <<'PY'
import json, os, pathlib, re, sys
from urllib.parse import unquote, urlsplit
root = pathlib.Path.cwd()
def read(path):
    return (root / path).read_text()
def check(label, expected, actual):
    if expected != actual:
        sys.exit(f'FAIL: {label}: missing={sorted(expected - actual)}, extra={sorted(actual - expected)}')
def arms(section):
    result = set()
    for line in section.splitlines():
        if '=>' in line:
            result.update(re.findall(r'"([a-z_]+)"', line.split('=>')[0]))
    return result
config = read('crates/clanky/src/config.rs').split('match key.as_str() {', 1)[1].split('_ =>', 1)[0]
keys = arms(config)
doc_keys = set(re.findall(r'^\| `([a-z_]+)` \|', read('docs/config.md'), re.MULTILINE))
if os.environ.get('CLANKY_GATE_FAULT') == 'docs':
    doc_keys.add('made_up_config_key')
check('config keys', keys, doc_keys)
main = read('crates/clanky/src/main.rs').split('    fn key(&mut self, key:', 1)[1].split('    fn click(', 1)[0]
shift = main.split('match lower.as_str() {', 1)[1].split('_ =>', 1)[0]
shortcuts = {'Ctrl+Shift+' + x.upper() for x in arms(shift)}
shortcuts.update('Ctrl+Shift+' + x.upper() for x in re.findall(r'eq_ignore_ascii_case\("([a-z])"\)', main))
zoom = main.split('match lower.as_str() {', 2)[2].split('_ =>', 1)[0]
shortcuts.update('Ctrl+' + x for x in re.findall(r'"([^\"]+)"', '\n'.join(line.split('=>')[0] for line in zoom.splitlines() if '=>' in line)))
if 'NamedKey::Tab' in main:
    shortcuts.add('Ctrl+Tab')
panels = read('crates/clanky/src/panels.rs').split('    pub fn key(', 1)[1]
panels = panels.split('\n    pub fn ', 1)[0]
shortcuts.update('Palette:' + x for x in re.findall(r'NamedKey::(\w+)', panels))
documented = set(re.findall(r'^\| `([^`]+)` \|', read('docs/shortcuts.md'), re.MULTILINE))
check('shortcuts', shortcuts, documented)
engine = read('crates/clanky-core/src/engine.rs')
actions = read('crates/clanky-core/src/actions.rs')
def operations(source, function, match_expr):
    return arms(source.split(function, 1)[1].split(match_expr, 1)[1].split('_ =>', 1)[0])
ops = {
    1: operations(engine, 'pub fn request(', 'match op {') | {'hello'},
    2: operations(actions, 'pub fn palette_action(', 'match action {') | {'hello'},
    3: operations(engine, 'pub fn owner_request(', 'match op {') | {'hello', 'auth'},
}
api = read('docs/agent-api.md')
examples = re.findall(r'^Request: `([^`]+)` Response: `([^`]+)`$', api, re.MULTILINE)
seen = {1: set(), 2: set(), 3: set()}
for request, response in examples:
    try:
        req, reply = json.loads(request), json.loads(response)
        version, op = req['v'], req['op']
        if version not in seen or op not in ops[version] or reply['id'] != req['id']:
            raise ValueError('unknown operation or mismatched response id')
        seen[version].add(op)
    except (ValueError, KeyError, TypeError) as exc:
        sys.exit(f'FAIL: API example {request}: {exc}')
for version in ops:
    check(f'v{version} operations', ops[version], seen[version])
for path in ['README.md', 'docs/getting-started.md', 'docs/config.md', 'docs/agent-api.md', 'docs/shortcuts.md']:
    text = read(path)
    for target in re.findall(r'(?<!!)\[[^\]]+\]\(([^)]+)\)', text):
        url = urlsplit(target.split()[0])
        if url.scheme or url.netloc or not url.path or url.path.startswith('/'):
            continue
        if not ((root / path).parent / unquote(url.path)).exists():
            sys.exit(f'FAIL: broken link in {path}: {target}')
print('PASS: documentation')
PY
