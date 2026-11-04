#!/usr/bin/env python3
"""Fixed, real-process M5 expectations. Diagnostics never contain observed data."""
import base64
import fcntl
import json
import os
from pathlib import Path
import secrets
import select
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
CHILDREN = []
TABS = []
RUN_COUNT = 0


class GateFailure(Exception):
    """Only fixed, source-controlled labels may be used here."""


def require(ok, label):
    if not ok:
        raise GateFailure(label)


def spawn(argv, **kw):
    p = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, **kw)
    CHILDREN.append(p)
    return p


def stop(p):
    if p.poll() is None:
        p.terminate()
    try:
        p.wait(timeout=3)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait(timeout=3)


class Pipe:
    def __init__(self, argv, **kw):
        self.p = spawn(argv, **kw)
        self.buf = bytearray()
        self.seq = 0

    def send(self, q):
        self.p.stdin.write(json.dumps(q).encode() + b'\n')
        self.p.stdin.flush()

    def recv(self):
        end = time.monotonic() + 30
        while b'\n' not in self.buf:
            left = end - time.monotonic()
            require(left > 0 and select.select([self.p.stdout], [], [], max(0, left))[0], 'reply deadline')
            chunk = os.read(self.p.stdout.fileno(), 65536)
            require(chunk, 'reply EOF')
            self.buf.extend(chunk)
            require(len(self.buf) <= 2 * 1024 * 1024, 'reply bound')
        line, _, rest = self.buf.partition(b'\n')
        self.buf = bytearray(rest)
        return json.loads(line)

    def start(self, op, args=None, v=4):
        self.send(dict(v=v, op=op, args=args or {}))

    def call(self, op, args=None, v=4):
        self.start(op, args, v)
        r = self.recv()
        require(r.get('ok') is True, 'RPC ' + op)
        return r['result']

    def denied(self, op, args=None, v=4):
        self.start(op, args, v)
        require(self.recv() == dict(ok=False, code='permission_denied'), 'denied ' + op)

    def rpc(self, method, params=None):
        self.seq += 1
        self.send(dict(jsonrpc='2.0', id=self.seq, method=method, params=params or {}))
        r = self.recv()
        require(r.get('id') == self.seq and 'result' in r, 'MCP ' + method)
        return r['result']

    def tool(self, name, args):
        r = self.rpc('tools/call', dict(name=name, arguments=args))
        require(len(r.get('content', [])) == 1, 'MCP text count')
        return r, json.loads(r['content'][0]['text'])


def until(fn, label):
    end = time.monotonic() + 10
    while time.monotonic() < end:
        value = fn()
        if value:
            return value
        time.sleep(.05)
    raise GateFailure(label)


def fixture(d, name, source):
    p = d / name
    p.write_text(source)
    p.chmod(0o600)
    return p


def tab(o, rows=24, cols=80, allowed=True):
    t = o.call('open_tab', dict(rows=rows, cols=cols, allow_agent=allowed), 1)['tab_id']
    TABS.append(t)
    return t


def run(o, t, argv, d):
    global RUN_COUNT
    RUN_COUNT += 1
    launcher = d / 'launch.py'
    if not launcher.exists():
        fixture(d, 'launch.py', "import os,sys\nopen(sys.argv[1],'w').write(str(os.getpid()))\nos.execvp(sys.argv[2],sys.argv[2:])\n")
    wrapped = [sys.executable, launcher, d / ('run-' + str(RUN_COUNT) + '.pid'), *argv]
    return o.call('run', dict(tab_id=t, argv=[str(x) for x in wrapped], cwd=str(d)), 1)['command_id']


def done(o, t, c, code=0):
    s = o.call('wait', dict(tab_id=t, command_id=c, timeout_ms=5000), 1)
    require(s == dict(done=True, state='exited', exit_code=code, signal=None), 'exit status')


def raw(o, t, c, expected):
    # Every page starts at a raw-stream coordinate, including inside a secret.
    out = bytearray()
    offset = 0
    for i in range(len(expected) + 1):
        r = o.call('get_output', dict(tab_id=t, command_id=c, offset=offset,
                                     limit=(i % 5) + 1, encoding='base64'), 1)
        b = base64.b64decode(r['data'], validate=True)
        require(r['next_offset'] == offset + len(b), 'raw coordinate')
        out.extend(b)
        offset = r['next_offset']
        require(r['retained_start'] == 0 and not r['truncated'], 'raw retention')
        if offset == len(expected):
            require(r['eof'] is True and r['total_bytes'] == len(expected), 'raw EOF')
            break
        require(b, 'raw progress')
    require(bytes(out) == expected, 'exact raw bytes')
    return bytes(out)


def events(a, after, t=None):
    q = dict(after=after, limit=128, timeout_ms=0)
    if t is not None:
        q['tab_id'] = t
    r = a.call('events', q)
    es = r['events']
    require(all(x['seq'] > after for x in es), 'event cursor origin')
    require(all(x['seq'] < y['seq'] for x, y in zip(es, es[1:])), 'event ordering')
    require(r['next_cursor'] >= after, 'event next cursor')
    return r


def head(a):
    cursor = 0
    for _ in range(10):  # 1024-entry ring, at most eight full pages plus EOF.
        page = events(a, cursor)
        following = page['next_cursor']
        if not page['events']:
            return following
        require(following > cursor, 'event page progress')
        cursor = following
    raise GateFailure('event page bound')


def block_command_masking(o, a, d, secret, fault=False):
    # The expected command is fixed before the prompt is opened or input is sent.
    expected = 'export X=' + '*' * len(secret)
    shell = ("printf '\\033]133;A\\007$ \\033]133;B\\007'\n"
             "IFS= read -r line\n"
             "printf '\\033]133;C\\007'\n"
             "eval \"$line\"\n"
             "printf '\\033]133;D;0\\007'\n")
    t = tab(o)
    c = run(o, t, ['sh', '-c', shell], d)
    until(lambda: a.call('input_state', dict(tab_id=t))['waiting'], 'block command prompt ready')
    a.call('paste', dict(tab_id=t, text='export X=' + secret.decode('ascii')))
    a.call('send_key', dict(tab_id=t, key='Enter'))
    done(o, t, c)
    response = a.call('list_blocks', dict(tab_id=t, limit=64), 1)
    blocks = response['blocks']
    require(len(blocks) == 1, 'block command count')
    observed = blocks[0]['command']
    if fault:
        observed = 'unmasked'
    require(observed == expected, 'block command masked')
    require(secret not in json.dumps(response).encode(), 'block command response no secret')


def owner_copy_search(o, tab_id, block_id, secret):
    # Fixed expectations: copying the secret's line yields stars; its first
    # twelve bytes cannot be found in the person's displayed scrollback.
    o.call('select', dict(tab_id=tab_id, block_id=block_id, start=0, end=len(secret)), 2)
    copied = o.call('copy', dict(tab_id=tab_id, block_id=block_id), 2)
    data = base64.b64decode(copied['data'], validate=True)
    search = o.call('window_search', dict(tab_id=tab_id, query=secret[:12].decode('ascii'),
                                          after=None, backwards=False), 3)
    require(data == b'*' * len(secret), 'owner copy masked')
    require(search['total'] == 0, 'owner search secret absent')


def suite(bins, d, env, fault):
    socket = d / 'server.sock'
    server = spawn([bins / 'clanky', '--server', '--socket', socket], env=env)
    until(lambda: socket.exists() or (server.poll() is not None and require(False, 'server startup')), 'socket startup')
    helper = bins / 'clanky-agent-gate'
    o = Pipe([helper, 'owner', socket], env=env)
    a = Pipe([helper, 'agent', socket], env=env)
    # Keep a terminal alive while independent fixtures close.
    keeper = tab(o)
    try:
        t = tab(o)
        c = run(o, t, ['sh', '-c', 'read x; echo got:$x'], d)
        time.sleep(.3)
        state = until(lambda: (s if (s := a.call('input_state', dict(tab_id=t)))['waiting'] else None), 'actual input wait')
        require(state['state'] == 'waiting' and state['evidence'] == 'pty_read', 'PTY evidence')
        w = a.call('wait_input', dict(tab_id=t, timeout_ms=1000))
        require(w['waiting'] and not w['timed_out'] and w['evidence'] == 'pty_read', 'bounded input wait')
        a.call('paste', dict(tab_id=t, text='hello'))
        a.call('send_key', dict(tab_id=t, key='Enter'))
        done(o, t, c)
        output = o.call('get_output', dict(tab_id=t, command_id=c, offset=0, limit=4096, encoding='base64'), 1)
        require(b'got:hello' in base64.b64decode(output['data']).splitlines(), 'completed input line')
        s = tab(o)
        sc = run(o, s, ['sh', '-c', 'sleep 600'], d)
        time.sleep(.3)
        require(a.call('input_state', dict(tab_id=s))['waiting'] is False, 'sleep is not input')
        sw = a.call('wait_input', dict(tab_id=s, timeout_ms=250))
        require(sw['timed_out'] is True and sw['waiting'] is False, 'sleep wait deadline')

        full = fixture(d, 'screen.py', "import os,time\nos.write(1,b'\\x1b[2J\\x1b[Halpha\\x1b[2;1Hbeta\\x1b[3;3H')\nwhile not os.path.exists('release-screen'): time.sleep(.02)\n")
        st = tab(o, 3, 8)
        run(o, st, [sys.executable, full], d)
        screen = until(lambda: (r if (r := a.call('screen', dict(tab_id=st)))['lines'] == ['alpha', 'beta', ''] else None), 'fixed screen')
        require(screen['cursor'] == [2, 2] and screen['rows'] == 3 and screen['cols'] == 8, 'fixed cursor')
        ht = tab(o, 3, 8)
        hc = run(o, ht, ['sh', '-c', "printf 'h0\\nh1\\nh2\\nh3\\nh4\\n'"], d)
        done(o, ht, hc)
        h = a.call('screen', dict(tab_id=ht, history_offset=0, history_limit=3))['history']
        require(h['lines'] == ['h0', 'h1', 'h2'] and h['offset'] == 0 and h['next_offset'] == 3, 'oldest history')

        keys = fixture(d, 'keys.py', "import os,tty\ntty.setraw(0)\nos.write(1,b'\\x1b[?2004h')\nopen('keys-ready','w').close()\nb=b''\nwhile len(b)<19: b+=os.read(0,19-len(b))\nopen('keys-bytes','wb').write(b)\n")
        kt = tab(o)
        kc = run(o, kt, [sys.executable, keys], d)
        until(lambda: (d / 'keys-ready').exists(), 'raw ready')
        until(lambda: a.call('input_state', dict(tab_id=kt))['bracketed_paste'] is True, 'paste mode parsed')
        for key, n in [('Up', 3), ('Ctrl-C', 1), ('Enter', 1)]:
            require(a.call('send_key', dict(tab_id=kt, key=key))['sent'] == n, 'key byte count')
        require(a.call('paste', dict(tab_id=kt, text='xy'))['sent'] == 14, 'bracketed byte count')
        done(o, kt, kc)
        require((d / 'keys-bytes').read_bytes() == b'\x1b[A\x03\r\x1b[200~xy\x1b[201~', 'literal key bytes')

        secret = env['CLANKY_TEST_SECRET'].encode()
        block_command_masking(o, a, d, secret, fault == 'block-command')
        token = ('s' + 'k' + '-' + ''.join(secrets.choice('abcdefghijklmnopqrstuvwxyz') for _ in range(20))).encode()
        (d / 'token').write_bytes(token)
        (d / 'token').chmod(0o600)
        maskprog = fixture(d, 'mask.py', "import os,time\na=os.environ['CLANKY_TEST_SECRET'].encode();b=open('token','rb').read()\nos.write(1,b'\\x1b]133;C\\x07')\nfor i,s in enumerate((a,b)):\n os.write(1,s[:7]);open('split-ready-'+str(i),'w').close()\n while not os.path.exists('split-release-'+str(i)): time.sleep(.02)\n os.write(1,s[7:]);os.write(1,b'\\n')\nos.write(1,b'\\x1b]133;D;0\\x07')\n")
        mt = tab(o)
        cursor = head(a)
        mc = run(o, mt, [sys.executable, maskprog], d)
        prefix, suffix = b'\x1b]133;C\x07', b'\x1b]133;D;0\x07'
        for i in range(2):
            until(lambda: (d / ('split-ready-' + str(i))).exists(), 'split write ready')
            time.sleep(.1)
            active = a.call('get_output', dict(tab_id=mt, command_id=mc, offset=0,
                                              limit=65536, encoding='base64'), 1)
            active_bytes = base64.b64decode(active['data'], validate=True)
            safe_prefix = prefix if i == 0 else prefix + b'*' * 24 + b'\r\n'
            require(safe_prefix.startswith(active_bytes) and not active['eof'], 'undecided secret withheld')
            (d / ('split-release-' + str(i))).touch()
        done(o, mt, mc)
        expected_owner = prefix + secret + b'\r\n' + token + b'\r\n' + suffix
        expected = prefix + b'*' * 24 + b'\r\n' + b'*' * 23 + b'\r\n' + suffix
        raw(o, mt, mc, expected_owner)
        observed = raw(a, mt, mc, expected)
        if fault == 'mask':
            observed = b'unmasked'
        require(observed == expected, 'fixed star masking')
        blocks = a.call('list_blocks', dict(tab_id=mt, limit=64), 1)['blocks']
        require(len(blocks) == 1 and blocks[0]['output'] == '*' * 24 + '\n' + '*' * 23, 'masked block')
        owner_copy_search(o, mt, blocks[0]['block_id'], secret)
        ms = a.call('screen', dict(tab_id=mt))
        require(ms['lines'][:2] == ['*' * 24, '*' * 23], 'masked screen')
        replay = events(a, cursor, mt)
        replay2 = events(a, cursor, mt)
        require(replay['events'] == replay2['events'], 'mask replay identical')
        payload = b''.join(base64.b64decode(e['data'], validate=True) for e in replay['events'] if e['kind'] == 'output_appended')
        require(payload == expected, 'masked replay bytes')

        et = tab(o)
        before = head(a)
        ec = run(o, et, ['sh', '-c', "printf 'event\\n'; exit 3"], d)
        done(o, et, ec, 3)
        er = events(a, before, et)
        require(er['events'] == events(a, before, et)['events'], 'identical event replay')
        out = [e for e in er['events'] if e['kind'] == 'output_appended']
        finish = [e for e in er['events'] if e['kind'] == 'command_finished']
        require(b''.join(base64.b64decode(e['data'], validate=True) for e in out) == b'event\r\n', 'event exact output')
        require(len(finish) == 1 and finish[0]['exit_code'] == 3 and out[-1]['seq'] < finish[0]['seq'], 'finish ordered')
        wt = tab(o)
        wc = run(o, wt, ['sh', '-c', 'read x'], d)
        until(lambda: a.call('input_state', dict(tab_id=wt))['waiting'], 'event wait fixture')
        wr = until(lambda: [e for e in events(a, before, wt)['events'] if e['kind'] == 'waiting_for_input'], 'waiting event')
        require(wr[-1]['evidence'] == 'pty_read', 'waiting event evidence')
        o.call('close_tab', dict(tab_id=et), 1)
        require(any(e['kind'] == 'terminal_closed' for e in events(a, before)['events']), 'closed event')
        # One acknowledged real PTY write per input prevents kernel coalescing
        # without relying on a monitor catching short-lived state transitions.
        overflow = fixture(d, 'overflow.py', "import os,tty\ntty.setraw(0)\nopen('overflow-ready','w').close()\nfor i in range(1025):\n os.read(0,1);os.write(1,b'!')\n")
        ot = tab(o)
        overflow_cursor = head(a)
        oc = run(o, ot, [sys.executable, overflow], d)
        until(lambda: (d / 'overflow-ready').exists(), 'overflow reader ready')
        for index in range(1025):
            a.call('paste', dict(tab_id=ot, text='x'))
            end = time.monotonic() + 10
            while True:
                require(time.monotonic() < end, 'overflow acknowledgement deadline')
                page = a.call('events', dict(after=overflow_cursor, tab_id=ot, limit=128, timeout_ms=1000))
                overflow_cursor = page['next_cursor']
                output = [e for e in page['events'] if e['kind'] == 'output_appended']
                if output:
                    require(b''.join(base64.b64decode(e['data']) for e in output) == b'!'
                            and output[-1]['offset'] == index + 1, 'overflow exact acknowledgement')
                    break
        done(o, ot, oc)
        require(events(a, 0)['gap'] is True, 'explicit ring gap')

        mcp = Pipe([bins / 'clanky', 'mcp', '--socket', socket], env=env)
        init = mcp.rpc('initialize', dict(protocolVersion='2024-11-05', capabilities={}, clientInfo=dict(name='fixed-gate', version='1')))
        require(init['protocolVersion'] == '2024-11-05', 'MCP version')
        mcp.send(dict(jsonrpc='2.0', method='notifications/initialized'))
        names = {x['name'] for x in mcp.rpc('tools/list')['tools']}
        require({'screen', 'input_state', 'wait_input', 'send_key', 'paste', 'events', 'get_output'} <= names and not {'audit', 'revoke_all', 'resume_agents'} & names, 'MCP agent tools')
        _, result = mcp.tool('screen', dict(tab_id=mt))
        require(result == ms, 'MCP screen equality')
        _, result = mcp.tool('get_output', dict(tab_id=mt, command_id=mc, offset=0, limit=65536, encoding='base64'))
        require(base64.b64decode(result['data']) == expected, 'MCP masked bytes')
        _, result = mcp.tool('events', dict(after=cursor, tab_id=mt, timeout_ms=0))
        require(result['gap'] is True, 'MCP replay gap')
        _, result = mcp.tool('input_state', dict(tab_id=wt))
        require(result['waiting'] and result['evidence'] == 'pty_read', 'MCP input state')
        _, result = mcp.tool('wait_input', dict(tab_id=wt, timeout_ms=1000))
        require(result['waiting'] and not result['timed_out'], 'MCP wait input')
        mcp.tool('paste', dict(tab_id=wt, text='mcp'))
        mcp.tool('send_key', dict(tab_id=wt, key='Enter'))
        done(o, wt, wc)
        denied = tab(o, allowed=False)
        for tool in ('screen', 'input_state'):
            r, result = mcp.tool(tool, dict(tab_id=denied))
            require(r.get('isError') is True and result.get('error', {}).get('code') == 'permission_denied', 'MCP denied terminal')

        a.call('paste', dict(tab_id=st, text=secret.decode()))
        audit = o.call('audit', dict(tab_id=st, after=0, limit=128))
        records = [r for r in audit['records'] if r['op'] == 'paste' and r['peer_pid'] == a.p.pid]
        require(records, 'audit actual peer')
        r = records[-1]
        require(r['time_unix_ms'] > 0 and r['connection_id'] > 0 and os.getpid() in r['ancestry'] and r['terminal_ancestry'] == [] and r['result_code'] == 'ok', 'audit identity fields')
        require(r['args']['text'] == '*' * 24, 'audit masked arguments')
        require(secret not in json.dumps(audit).encode() and token not in json.dumps(audit).encode(), 'audit no secret')
        # A second real peer is descended from a server-owned PTY. Its target
        # tab is different, so terminal ancestry must come from captured PIDs,
        # never the request's target tab_id.
        inside = fixture(d, 'inside.py',
                         "import json,subprocess,sys\n"
                         "p=subprocess.Popen(sys.argv[1:4],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)\n"
                         "open('inside-peer','w').write(str(p.pid))\n"
                         "q=dict(v=4,op='screen',args=dict(tab_id=sys.argv[4]))\n"
                         "p.communicate(json.dumps(q).encode()+b'\\n',timeout=10)\n")
        it = tab(o)
        ic = run(o, it, [sys.executable, inside, helper, 'agent', socket, st], d)
        done(o, it, ic)
        inside_pid = int((d / 'inside-peer').read_text())
        inside_audit = o.call('audit', dict(tab_id=st, after=0, limit=128))
        inside_records = [r for r in inside_audit['records'] if r['peer_pid'] == inside_pid and r['op'] == 'screen']
        require(len(inside_records) == 1 and inside_records[0]['terminal_ancestry'] == [it]
                and inside_records[0]['result_code'] == 'ok', 'audit actual terminal ancestry')
        pending_input = Pipe([helper, 'agent', socket], env=env)
        pending_event = Pipe([helper, 'agent', socket], env=env)
        pending_v1 = Pipe([helper, 'agent', socket], env=env)
        pending_input.start('wait_input', dict(tab_id=s, timeout_ms=5000))
        pending_event.start('events', dict(after=head(a), tab_id=s, timeout_ms=5000))
        pending_v1.start('wait', dict(tab_id=s, command_id=sc, timeout_ms=5000), 1)
        time.sleep(.2)
        stopped = o.call('revoke_all')
        require(stopped['revoked'] > 0 and stopped['epoch'] > 0, 'revocation epoch')
        for p in (pending_input, pending_event, pending_v1):
            require(p.recv() == dict(ok=False, code='permission_denied'), 'cancel pending wait')
        for t in TABS:
            if t != et:
                a.denied('screen', dict(tab_id=t))
        a.denied('open_tab', dict(allow_agent=True), 1)
        raw(o, mt, mc, expected_owner)
        o.call('resume_agents')
        a.denied('screen', dict(tab_id=mt))
    finally:
        for t in reversed(TABS):
            try:
                o.call('close_tab', dict(tab_id=t), 1)
            except Exception:
                pass


def cleanup_groups(d):
    # PTY roots are session/process-group leaders. Signal only the numeric
    # groups written by our exec wrapper, never a process-name pattern. This
    # also covers descendants if an RPC transport failed before close_tab.
    groups = []
    for path in d.glob('run-*.pid'):
        try:
            pid = int(path.read_text())
            if pid > 1 and pid != os.getpgrp():
                groups.append(pid)
                os.killpg(pid, 15)
        except (OSError, ValueError):
            pass
    deadline = time.monotonic() + 3
    while groups and time.monotonic() < deadline:
        remaining = []
        for pid in groups:
            try:
                os.killpg(pid, 0)
                remaining.append(pid)
            except ProcessLookupError:
                pass
        groups = remaining
        if groups:
            time.sleep(.05)
    for pid in groups:
        try:
            os.killpg(pid, 9)
        except ProcessLookupError:
            pass


def main():
    os.umask(0o077)
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'mask', 'block-command'), 'fault selection')
    env = os.environ.copy()
    # Only generated credentials are test inputs; preserve operational paths,
    # but do not import the owner's secret-bearing environment into fixtures.
    for name in list(env):
        if any(word in name.upper() for word in ('KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD')):
            env.pop(name)
    target = Path(env.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    lock = Path(env.get('CLANKY_BUILD_LOCK', target / '.build.lock')).resolve()
    env.update(CARGO_TARGET_DIR=str(target), CLANKY_BUILD_LOCK=str(lock))
    lock.parent.mkdir(parents=True, exist_ok=True)
    # Keep scratch and binaries alongside the configured build output, not /tmp.
    with tempfile.TemporaryDirectory(prefix='m5-gate-', dir=target.parent) as work:
        d = Path(work)
        bins = d / 'bins'
        bins.mkdir()
        with lock.open('a') as f:
            fcntl.flock(f, fcntl.LOCK_EX)
            for p in (target / 'debug').iterdir():
                if p.is_file() and os.access(p, os.X_OK):
                    shutil.copy2(p, bins / p.name)
        require((bins / 'clanky-agent-gate').exists(), 'helper captured')
        # Actual server receives a synthetic policy literal; it is not just a
        # fixture-local variable. No owner capability enters this environment.
        env['CLANKY_TEST_SECRET'] = ''.join(secrets.choice('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ') for _ in range(24))
        try:
            suite(bins, d, env, fault)
        finally:
            cleanup_groups(d)
            for p in reversed(CHILDREN):
                stop(p)
        if not fault:
            regression_env = env.copy()
            regression_env.update(CARGO_TARGET_DIR=str(target), CLANKY_BUILD_LOCK=str(lock), CLANKY_GATE_BINARIES_READY='1')
            regression_env.pop('CLANKY_GATE_FAULT', None)
            for name in ('gate-m4.sh', 'gate-m6-masking.sh', 'gate-m7-core.sh', 'gate-linux-packaging.sh'):
                require(subprocess.run(['bash', ROOT / 'scripts' / name], cwd=ROOT, env=regression_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=7200).returncode == 0, 'regression ' + name)
    print('PASS: M5 agent control and safety')


if __name__ == '__main__':
    try:
        main()
    except BaseException as error:
        for child in reversed(CHILDREN):
            stop(child)
        # Never print exception text, request bodies, replies, or secret values.
        label = str(error) if isinstance(error, GateFailure) else 'fixture setup or transport'
        print('FAIL: M5 ' + label, file=sys.stderr)
        sys.exit(1)
