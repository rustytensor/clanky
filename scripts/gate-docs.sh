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
        sys.exit(f'FAIL: {label}: missing={sorted(expected-actual)}, extra={sorted(actual-expected)}')
def require(ok, message):
    if not ok:
        sys.exit('FAIL: ' + message)
# These inventories read Rust dispatch/key handlers, not a second feature list.
# Skip quoted braces and comments when delimiting functions/matches. This is a
# lexical inventory of the current handlers, not a general Rust semantic parser.
TOKEN = re.compile(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'|[{}()]', re.S)
def body_at(text, opening):
    depth = 0
    for token in TOKEN.finditer(text, opening):
        value = token.group()
        if value == '{': depth += 1
        elif value == '}':
            depth -= 1
            if depth == 0: return text[opening+1:token.start()]
    raise ValueError('unterminated source block')
def function(text, name):
    match = re.search(r'\bfn\s+' + re.escape(name) + r'\s*(?:<[^\n]*?>)?\s*\(', text)
    if not match: raise ValueError('source function missing: ' + name)
    return body_at(text, text.index('{', match.end()))
def blocks(text, pattern):
    return [body_at(text, m.end()-1) for m in re.finditer(pattern, text)]
def depth_at(text, end):
    depth = 0
    for token in TOKEN.finditer(text, 0, end):
        if token.group() in ('{','('): depth += 1
        elif token.group() in ('}',')'): depth -= 1
    return depth
def arm_details(text):
    pattern = r'(?m)^[ \t]*((?:"[^"\n]+"[ \t]*(?:\|[ \t]*)?)+)([^\n]*?)=>'
    hits = [m for m in re.finditer(pattern, text) if depth_at(text,m.start()) == 0]
    for n,m in enumerate(hits):
        yield re.findall(r'"([^"\n]+)"', m.group(1)), m.group(2), text[m.end():hits[n+1].start() if n+1<len(hits) else len(text)]
def arms(text):
    return {name for names,_,_ in arm_details(text) for name in names}
def operations(source, name, variable):
    return set().union(*(arms(b) for b in blocks(function(source,name),r'\bmatch\s+'+variable+r'\s*\{')))
def named(text):
    return set(re.findall(r'NamedKey::(\w+)',text))

fault = os.environ.get('CLANKY_GATE_FAULT','')
require(fault in ('','docs'), 'unknown documentation fault')
config = function(read('crates/clanky/src/config.rs'),'from_json')
keys = arms(blocks(config,r'match key\.as_str\(\)\s*\{')[0])
doc_keys = set(re.findall(r'^\| `([a-z0-9_]+)` \|',read('docs/config.md'),re.M))
if fault == 'docs': doc_keys.add('made_up_config_key')
check('config keys',keys,doc_keys)

main = function(read('crates/clanky/src/main.rs'),'key')
letter_matches = blocks(main,r'match lower\.as_str\(\)\s*\{')
shortcuts = {'Ctrl+Shift+'+x.upper() for x in arms(letter_matches[0])}
settings_blocks = blocks(main, r'if\s+self\.settings_open\(\)\s*\{')
require(len(settings_blocks) == 1, 'expected one settings modal key block')
terminal_keys = main.replace(settings_blocks[0], '', 1)
shortcuts.update('Ctrl+Shift+'+x.upper() for x in re.findall(r'eq_ignore_ascii_case\("([a-z])"\)',terminal_keys))
shortcuts.update('Ctrl+'+x for x in arms(letter_matches[1]))
shortcuts.update(('Ctrl+' if x=='Tab' else 'Ctrl+Shift+')+x for x in named(main))
# Modal and card handlers have their own contexts, not terminal chords.
shortcuts.update('Ctrl+'+x for x in re.findall(r'control_key\(\)[^\n]*?ch\s*==\s*"([^"]+)"', terminal_keys))
shortcuts.update('Settings:Ctrl+'+x.upper() for x in re.findall(r'eq_ignore_ascii_case\("([a-z])"\)', settings_blocks[0]))
shortcuts.update('Settings:'+x for x in named(function(read('crates/clanky/src/settings_panel.rs'),'key')))
card = function(read('crates/clanky/src/studio.rs'),'card_key')
card_match = blocks(card, r'\bmatch\s+key\s*\{')
require(len(card_match) == 1, 'expected one card key match')
shortcuts.update(named(card[:card.index('match key')]))
shortcuts.update('Card:'+x for x in named(card_match[0]))
stop = main.index('"v4.revoke_all"')
require(all(x in main[:stop] for x in ('control_key()','shift_key()','NamedKey::Escape'))
        and stop < main.index('self.compose.active_target()')
        and stop < main.index('self.panels.open'), 'revoke shortcut must precede modal routing')
for prefix,path,method in [
    ('Palette','crates/clanky/src/panels.rs','key'),
    ('File prompt','crates/clanky/src/editor_workspace.rs','prompt_key'),
    ('Editor','crates/clanky/src/editor.rs','key'),
    ('Compose','crates/clanky/src/compose.rs','handle_key'),
    ('Search','crates/clanky/src/scrollback_search.rs','key')]:
    code = function(read(path),method)
    shortcuts.update(prefix+':'+x for x in named(code))
    if prefix == 'Editor':
        for movement in blocks(code,r'match key\s*\{'):
            if 'Some("left")' in movement:
                shortcuts.update(prefix+':Shift+'+x for x in named(movement))
        for names,guard,action in arm_details(blocks(code,r'match lower\.as_str\(\)\s*\{')[0]):
            alt = 'modifiers.alt_key()' in guard and '!modifiers.alt_key()' not in guard
            base = 'Ctrl+' + ('Alt+' if alt else '')
            for name in names:
                shortcuts.add(prefix+':'+base+name.upper())
                if 'if extend' in action:
                    shortcuts.add(prefix+':'+base+'Shift+'+name.upper())
    elif prefix == 'Compose':
        for m in re.finditer(r'eq_ignore_ascii_case\("([a-z])"\)([^\n{]*)\{',code):
            modifiers = 'Ctrl+Alt+' if 'alt_key()' in m.group(2) else 'Ctrl+'
            shortcuts.add(prefix+':'+modifiers+m.group(1).upper())
        if 'NamedKey::Enter' in code and re.search(r'modifiers\.control_key\(\)\s*&&\s*!modifiers\.shift_key\(\)',code):
            shortcuts.add(prefix+':Ctrl+Enter')
    elif prefix == 'Search' and 'mods.shift_key()' in code:
        shortcuts.add(prefix+':Shift+Enter')
# The text-mode editor has its own input vocabulary, not winit's NamedKey.
tui = function(read('crates/clanky/src/tui.rs'),'key')
shortcuts.update('TUI:F'+n for n in re.findall(r'KeyCode::F\((\d+)\)',tui))
for name in set(re.findall(r'KeyCode::(\w+)',tui))-{'Char','F'}:
    shortcuts.add('TUI:'+{'Esc':'Escape','BackTab':'Shift+Tab'}.get(name,name))
confirm = blocks(tui,r'if self\.confirm\s*\{')[0]
shortcuts.update('TUI:'+c.upper() for c in re.findall(r"'([ynYN])'",confirm))
for condition in re.findall(r'if ctrl[^\n{]*',tui):
    shortcuts.update('TUI:Ctrl+'+c.upper() for c in re.findall(r"'([a-z])'",condition))
for block in blocks(tui,r'if ctrl\s*\{'):
    shortcuts.update('TUI:Ctrl+'+c.upper() for c in re.findall(r"KeyCode::Char\('([a-z])'\)",block))
for movement in re.findall(r'((?:KeyCode::\w+\s*(?:\|\s*)?)+)=>\s*self\.move_cursor\(',tui):
    shortcuts.update('TUI:Shift+'+x for x in re.findall(r'KeyCode::(\w+)',movement))
documented = set(re.findall(r'^\| `([^`]+)` \|',read('docs/shortcuts.md'),re.M))
check('shortcuts',shortcuts,documented)

engine=read('crates/clanky-core/src/engine.rs')
agent=read('crates/clanky-core/src/agent.rs')
transport=function(read('crates/clanky-api/src/lib.rs'),'dispatch')
ops={1:operations(engine,'request','op'),
     2:operations(read('crates/clanky-core/src/actions.rs'),'palette_action','action'),
     3:operations(engine,'owner_request','op'),
     4:operations(agent,'agent_request','op')}
# Discover transport-level hello/auth/focus_set rather than overlooking them.
for m in re.finditer(r'if op == "([a-z_]+)"',transport):
    if depth_at(transport,m.start()) == 0:
        for names in ops.values(): names.add(m.group(1))
for version in ops:
    for block in blocks(transport,r'if version == '+str(version)+r'\s*\{'):
        ops[version].update(re.findall(r'if op == "([a-z_]+)"',block))
mcp_source=read('crates/clanky/src/mcp.rs')
mcp_tools=set(re.findall(r'\btool\(\s*"([a-z_]+)"',function(mcp_source,'tools')))
require(bool(mcp_tools) and all(ops.values()),'empty source operation inventory')
api=read('docs/agent-api.md')
examples=re.findall(r'^Request: `([^`]+)` Response: `([^`]+)`$',api,re.M)
seen={version:set() for version in ops}
seen_tools=set()
for request,response in examples:
    try:
        req,reply=json.loads(request),json.loads(response)
        if reply['id'] != req['id']: raise ValueError('response id differs')
        if 'v' in req:
            version,op=req['v'],req['op']
            if version not in ops or op not in ops[version]: raise ValueError('unknown socket operation')
            if op in seen[version]: raise ValueError('duplicate socket example')
            if not isinstance(reply['ok'],bool): raise ValueError('missing result status')
            if ('result' if reply['ok'] else 'error') not in reply: raise ValueError('missing payload')
            seen[version].add(op)
        else:
            if req['jsonrpc']!='2.0' or reply['jsonrpc']!='2.0' or req['method']!='tools/call':
                raise ValueError('expected MCP tools/call')
            name=req['params']['name']
            if name not in mcp_tools or name in seen_tools: raise ValueError('unknown/duplicate MCP tool')
            if not isinstance(req['params']['arguments'],dict): raise ValueError('invalid MCP arguments')
            result=reply['result']
            if not isinstance(result['isError'],bool): raise ValueError('invalid MCP status')
            for content in result['content']:
                if content['type']!='text': raise ValueError('expected MCP text content')
                json.loads(content['text'])
            seen_tools.add(name)
    except (ValueError,KeyError,TypeError) as exc:
        sys.exit(f'FAIL: API example: {exc}')
for version in ops: check(f'v{version} operations',ops[version],seen[version])
check('MCP tools',mcp_tools,seen_tools)
for path in ['README.md','docs/getting-started.md','docs/config.md','docs/agent-api.md','docs/shortcuts.md','docs/design-docs-1.0.md']:
    text=read(path)
    require('coming in 1.0' not in text.lower(),f'obsolete release claim in {path}')
    for target in re.findall(r'(?<!!)\[[^\]]+\]\(([^)]+)\)',text):
        url=urlsplit(target.split()[0])
        if url.scheme or url.netloc or not url.path or url.path.startswith('/'): continue
        require(((root/path).parent/unquote(url.path)).exists(),f'broken link in {path}: {target}')
print('PASS: documentation (config, shortcuts, v1-v4, MCP tools and links)')
PY
