#!/usr/bin/env python3
"""H1 saved history: real server, real shell, values fixed in docs/design-h1-history.md.

Usage: gate-h1-history.py [--fault secret|restart|actor|delete]
A fault tampers with the run on purpose; the gate must then FAIL.
Diagnostics are fixed labels only; observed text is never printed.
"""
import base64, csv, hashlib, io, json, os, secrets, shutil, socket, sqlite3, stat, subprocess, sys, tempfile, time
from pathlib import Path

TARGET = Path(os.environ['CARGO_TARGET_DIR'])
APP = TARGET / 'debug/clanky'
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
KEYS = ['id', 'session_id', 'started_at', 'cwd', 'actor', 'command', 'exit_code', 'duration_ms',
        'complete', 'output', 'output_truncated']
DAY = 86_400_000
LOOP = ("while :; do printf '\\033]133;A\\007$ \\033]133;B\\007'; IFS= read -r line || exit 0; "
        "printf '\\033]133;C\\007'; eval \"$line\"; s=$?; printf '\\033]133;D;%s\\007' \"$s\"; done")


class Fail(Exception):
    pass


SERVERS = []


def require(ok, label):
    if not ok:
        raise Fail(label)


def until(fn, label, timeout=15):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        v = fn()
        if v:
            return v
        time.sleep(.05)
    raise Fail(label)


class Conn:
    def __init__(self, path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.settimeout(30)
        self.s.connect(str(path))
        self.r = self.s.makefile('rb')
        self.seq = 0

    def call(self, v, op, **args):
        self.seq += 1
        self.s.sendall((json.dumps(dict(v=v, id=str(self.seq), op=op, **args)) + '\n').encode())
        reply = json.loads(self.r.readline(4 << 20))
        require(reply.get('ok') is True, 'RPC ' + op)
        return reply['result']

    def close(self):
        self.r.close()
        self.s.close()


class Server:
    """One headless server on a private socket; stop() sends SIGTERM and waits."""

    def __init__(self, w, env, config):
        self.sock = w / 'server.sock'
        self.p = subprocess.Popen([APP, '--server', '--socket', self.sock, '--config', config], env=env,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        SERVERS.append(self.p)
        until(lambda: self.sock.exists() or require(self.p.poll() is None, 'server startup'), 'socket startup')
        self.owner = Conn(self.sock)
        self.owner.call(3, 'hello')
        self.owner.call(3, 'auth', token=Path(str(self.sock) + '.owner').read_text().strip())
        self.agent = Conn(self.sock)
        self.agent.call(4, 'hello', client_name='rusty')
        self.tab = self.agent.call(1, 'open_tab', allow_agent=True)['tab_id']
        self.agent.call(1, 'run', tab_id=self.tab, argv=['/bin/sh', '-c', LOOP], cwd=str(w))
        self.blocks = 0

    def waiting(self):
        return self.agent.call(4, 'input_state', tab_id=self.tab)['waiting']

    def type(self, line, by='you', wait=True):
        until(self.waiting, 'prompt ready')
        if by == 'you':
            self.owner.call(3, 'input', tab_id=self.tab, data=base64.b64encode(line.encode() + b'\r').decode())
        else:
            self.agent.call(4, 'paste', tab_id=self.tab, text=line)
            self.agent.call(4, 'send_key', tab_id=self.tab, key='Enter')
        self.blocks += 1
        if wait:
            until(lambda: (b := self.agent.call(1, 'list_blocks', tab_id=self.tab, limit=64)['blocks'])
                  and len(b) >= min(self.blocks, 64) and b[-1]['complete'], 'block complete')

    def stop(self):
        for c in (self.owner, self.agent):
            c.close()
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait(timeout=5)
            raise Fail('server stops within 10 s of SIGTERM')


def setup(root, name, **cfg):
    w = root / name
    w.mkdir(mode=0o700)
    for d in ('home', 'config', 'data', 'state', 'runtime'):
        (w / d).mkdir(parents=True, mode=0o700)
    env = {k: v for k, v in os.environ.items() if not k.startswith(('XDG_', 'CLANKY'))}
    env.update(HOME=str(w / 'home'), XDG_CONFIG_HOME=str(w / 'config'), XDG_DATA_HOME=str(w / 'data'),
               XDG_STATE_HOME=str(w / 'state'), XDG_RUNTIME_DIR=str(w / 'runtime'), SHELL='/bin/sh',
               CLANKY_TEST_SECRET=SECRET)
    config = w / 'config.json'
    config.write_text(json.dumps(dict(restore_layout=False, startup_animation=False, **cfg)))
    return w, env, config, w / 'data/clanky/history.sqlite3'


def cli(env, *args, code=0):
    r = subprocess.run([APP, 'history', *map(str, args)], env=env, capture_output=True, timeout=30)
    require(r.returncode == code, 'history ' + str(args[0]) + ' exit code')
    return r.stdout


def rows(env, *args):
    out = json.loads(cli(env, *args, '--json'))
    require(isinstance(out, list) and all(isinstance(r, dict) and list(r) == KEYS for r in out), 'JSON row keys')
    return out


def file_bytes(db):
    return b''.join(p.read_bytes() for p in (db, Path(str(db) + '-wal'), Path(str(db) + '-shm')) if p.exists())


def tamper(db, sql, *args):
    with sqlite3.connect(db) as c:
        c.execute(sql, args)
    sqlite3.connect(db).execute('PRAGMA wal_checkpoint(TRUNCATE)').close()


def main_story(root):
    w, env, config, db = setup(root, 'main', history_output_kib=1)
    m = secrets.token_hex(4)
    t0 = int(time.time() * 1000)
    # Expected rows, written down before anything runs: (actor, command, exit, complete, output, truncated).
    want = [('you', f"printf 'alpha-{m}\\n'", 0, True, f'alpha-{m}', False),
            ('you', "sh -c 'exit 3'", 3, True, '', False),
            ('you', "printf '%s\\n' " + '*' * len(TOKEN), 0, True, '*' * len(TOKEN), False),
            ('you', 'printf \'%s\\n\' "$CLANKY_TEST_SECRET"', 0, True, '*' * len(SECRET), False),
            ('rusty', f"printf 'beta-{m}\\n'", 0, True, f'beta-{m}', False),
            ('you', 'yes xxxxxxxxx | head -n 1500', 0, True, ('xxxxxxxxx\n' * 103)[:1024], True),
            ('you', 'sleep 30', None, False, '', False),
            ('you', f"printf 'gamma-{m}\\n'", 0, True, f'gamma-{m}', False)]
    s = Server(w, env, config)
    for actor, command, *_ in want[:6]:
        typed = f"printf '%s\\n' {TOKEN}" if command.startswith("printf '%s\\n' *") else command
        s.type(typed, by=actor)
    s.type('sleep 30', wait=False)
    time.sleep(1)
    s.stop()
    if FAULT == 'restart':
        for p in db.parent.glob('history.sqlite3*'):
            p.unlink()
    if FAULT == 'secret':
        tamper(db, 'UPDATE commands SET output = ? WHERE id = (SELECT min(id) FROM commands)', TOKEN)
    if FAULT == 'actor':
        tamper(db, "UPDATE commands SET actor = 'you' WHERE actor = 'rusty'")
    s = Server(w, env, config)
    s.type(want[7][1])
    s.stop()
    t1 = int(time.time() * 1000)

    # The file itself, through Python's own SQLite (an outside tool).
    require(stat.S_IMODE(db.stat().st_mode) == 0o600 and stat.S_IMODE(db.parent.stat().st_mode) == 0o700, 'file modes')
    with sqlite3.connect(f'file:{db}?mode=ro', uri=True) as c:
        require(c.execute('PRAGMA integrity_check').fetchone()[0] == 'ok', 'integrity check')
        require(c.execute('PRAGMA user_version').fetchone()[0] == 1, 'schema version 1')
        sessions = c.execute('SELECT started_at, ended_at FROM sessions ORDER BY id').fetchall()
        events = c.execute("SELECT action, detail FROM agent_events WHERE agent = 'rusty'").fetchall()
    require(len(sessions) == 2 and all(e is not None and e >= st for st, e in sessions), 'two sessions, both ended')
    require({'paste', 'send_key'} <= {a for a, _ in events}, 'agent timeline saved')

    got = rows(env, 'list', '--limit', 1000)
    require(len(got) == 8, 'eight rows after restart')
    require([(r['actor'], r['command'], r['exit_code'], r['complete'], r['output'], r['output_truncated'])
             for r in got] == want, 'rows match expected')
    require(all(r['cwd'] == str(w) for r in got), 'cwd')
    require(len({r['session_id'] for r in got[:7]}) == 1 and got[7]['session_id'] != got[0]['session_id'], 'sessions')
    starts = [r['started_at'] for r in got]
    require(starts == sorted(starts) and t0 <= starts[0] and starts[-1] <= t1, 'start times')
    require(all(isinstance(r['duration_ms'], int) and r['duration_ms'] >= 0 for i, r in enumerate(got) if i != 6)
            and got[6]['duration_ms'] is None, 'durations')
    ids = [r['id'] for r in got]

    require([r['id'] for r in rows(env, 'search', f'beta-{m}')] == [ids[4]], 'search finds one row')
    require([r['id'] for r in rows(env, 'search', f'alpha-{m}')] == [ids[0]], 'search finds alpha')
    require(rows(env, 'search', TOKEN[:12]) == [], 'search never finds a secret')
    require(f'alpha-{m}'.encode() in cli(env, 'show', ids[0]), 'show')
    exports = {f: cli(env, 'export', '--format', f) for f in ('json', 'csv', 'markdown')}
    require(json.loads(exports['json']) == got, 'JSON export equals list')
    table = list(csv.reader(io.StringIO(exports['csv'].decode())))
    require(table[0] == KEYS and len(table) == 9 and table[5][KEYS.index('output')] == f'beta-{m}', 'CSV export')
    require(f'beta-{m}'.encode() in exports['markdown'], 'Markdown export')
    out = root / 'export.json'
    cli(env, 'export', '--format', 'json', '--output', out)
    require(json.loads(out.read_text()) == got, 'export to file')
    require(cli(env, 'path').decode().strip() == str(db), 'path')

    everything = file_bytes(db) + b''.join(exports.values())
    require(TOKEN.encode() not in everything and SECRET.encode() not in everything, 'no secret on disk or in exports')

    if FAULT != 'delete':
        cli(env, 'delete', ids[4])
    require([r['id'] for r in rows(env, 'list', '--limit', 1000)] == ids[:4] + ids[5:], 'delete removes one row')
    require(rows(env, 'search', f'beta-{m}') == [], 'deleted row not searchable')
    require(f'beta-{m}'.encode() not in file_bytes(db), 'deleted text gone from file bytes')

    # Retention: 100 days old goes, 89 days old stays (history_days 90).
    now = int(time.time() * 1000)
    tamper(db, 'UPDATE commands SET started_at = ? WHERE id = ?', now - 100 * DAY, ids[0])
    tamper(db, 'UPDATE commands SET started_at = ? WHERE id = ?', now - 89 * DAY, ids[7])
    config.write_text(json.dumps(dict(restore_layout=False, startup_animation=False, history_days=90)))
    s = Server(w, env, config)
    until(lambda: ids[0] not in [r['id'] for r in rows(env, 'list', '--limit', 1000)], 'retention deletes old row')
    s.stop()
    left = [r['id'] for r in rows(env, 'list', '--limit', 1000)]
    require(ids[7] in left and ids[0] not in left, 'retention keeps 89-day row')
    cli(env, 'clear', '--before', '2000-01-01')
    require(len(rows(env, 'list', '--limit', 1000)) == len(left), 'clear --before keeps newer rows')
    cli(env, 'clear')
    require(rows(env, 'list', '--limit', 1000) == [], 'clear empties history')
    cli(env, 'delete', 'nonsense', code=2)


def off(root):
    w, env, config, db = setup(root, 'off', history=False)
    s = Server(w, env, config)
    s.type('true')
    s.stop()
    require(not db.exists() and not list((w / 'data').rglob('history*')), 'history off creates no file')


def broken(root):
    w, env, config, db = setup(root, 'broken')
    db.parent.mkdir(parents=True, mode=0o700)
    junk = b'not a database\n' * 100
    db.write_bytes(junk)
    s = Server(w, env, config)
    s.type('true')
    s.stop()
    kept = list(db.parent.glob('history.sqlite3.broken-*'))
    require(len(kept) == 1 and kept[0].read_bytes() == junk, 'broken file kept aside unchanged')
    require(len(rows(env, 'list')) == 1, 'fresh file after broken one')


def newer(root):
    w, env, config, db = setup(root, 'newer')
    db.parent.mkdir(parents=True, mode=0o700)
    with sqlite3.connect(db) as c:
        c.execute('CREATE TABLE future (x)')
        c.execute('PRAGMA user_version = 99')
    db.chmod(0o600)
    before = hashlib.sha256(db.read_bytes()).hexdigest()
    s = Server(w, env, config)
    s.type('true')
    s.stop()
    require(hashlib.sha256(db.read_bytes()).hexdigest() == before, 'newer file left untouched')


def burst(root):
    w, env, config, db = setup(root, 'burst')
    s = Server(w, env, config)
    until(s.waiting, 'prompt ready')
    s.owner.call(3, 'input', tab_id=s.tab, data=base64.b64encode(b'true\r' * 300).decode())
    slowest = 0
    def count():
        nonlocal slowest
        t = time.monotonic()
        s.agent.call(1, 'list_tabs')
        slowest = max(slowest, time.monotonic() - t)
        if not db.exists():
            return False
        with sqlite3.connect(f'file:{db}?mode=ro', uri=True) as c:
            return c.execute('SELECT count(*) FROM commands').fetchone()[0] >= 300
    until(count, '300 rows written while the server runs', timeout=30)
    s.stop()
    require(slowest < .25, 'server answers within 250 ms while writing')


TOKEN = 's' + 'k-' + ''.join(secrets.choice('abcdefghijklmnopqrstuvwxyz') for _ in range(20))
SECRET = ''.join(secrets.choice('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ') for _ in range(24))

if __name__ == '__main__':
    with tempfile.TemporaryDirectory(prefix='h1-history-', dir=TARGET) as tmp:
        root = Path(tmp)
        failed = 0
        for check in (main_story, off, broken, newer, burst):
            try:
                check(root)
                print('PASS:', check.__name__)
            except Fail as e:
                failed += 1
                print('FAIL:', check.__name__, '-', e)
            except Exception as e:  # noqa: BLE001 - type only, never the observed text
                failed += 1
                print('FAIL:', check.__name__, '- error', type(e).__name__)
            finally:
                for p in SERVERS:
                    if p.poll() is None:
                        p.kill()
                        p.wait(timeout=5)
                SERVERS.clear()
        sys.exit(1 if failed else 0)
