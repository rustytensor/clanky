#!/usr/bin/env python3
"""A1 gate: start an agent's own command-line tool in a new pane (mockup approved by Marc 2026-11-12).

Usage: gate-a1-agent-pane.py [--fault missing|access|post-ai|network|picker|window-post-ai]
A fault tampers with the run on purpose; the gate must then FAIL. picker and window-post-ai run only A8, on
clanky-gpui: picker lists the window's agents in reverse order, window-post-ai starts the Post-AI window without
Post-AI.

Interface fixed by this gate (the work must provide it):
  clanky agent [--socket PATH] [--config PATH] [--tab] NAME [DIR]
      NAME matches an `agents` entry by name, case-insensitive, or by command. DIR defaults to the current
      directory. Opens a new terminal in the running Clanky on --socket, starts a shell there in DIR and runs the
      tool's command in it. Exit 0 on success; 1 with "not installed" on stderr when the command is not on PATH
      (no terminal opens); 2 in Post-AI mode or for bad arguments (no terminal opens).
  clanky agent [--config PATH] --list
      One line per configured agent: "<name>\t<command>\t<found|not installed>", in config order.
  config "agents": [{"name": ..., "command": ...}, ...]; when absent, the defaults are Claude Code/claude,
      Codex/codex, Pi/pi. config "agent_pane_access": "read+run" (default) or "off".
  In the window: Ctrl+Shift+G opens the picker; Enter starts the highlighted (first found) agent.

Expected results, fixed before the work:
  A1 `clanky agent fake DIR` -> exit 0; exactly one new terminal; it has allow_agent true and the terminal that
     existed before keeps allow_agent false; its screen shows "FAKE-AGENT cwd=DIR args=" within 10 s.
  A2 after the tool exits, the same terminal still runs a shell: typed "echo SHELL-$((40+2))" shows "SHELL-42".
  A3 `clanky agent missing` -> exit 1, "not installed" on stderr, terminal count unchanged.
  A4 `--list` with the gate config prints exactly the two lines below; with no "agents" key and only a `claude`
     stub on PATH it prints the three default lines below.
  A5 "agent_pane_access": "off" -> the new terminal has allow_agent false.
  A6 Post-AI server -> `clanky agent fake` exits 2 and no terminal opens.
  A7 strace of the server and the CLI during A1 shows no AF_INET or AF_INET6 connect().
  A8 window (private Xvfb): Ctrl+Shift+G then Enter opens one new terminal with allow_agent true showing
     "FAKE-AGENT"; in a Post-AI window Ctrl+Shift+G opens nothing within 3 s.
  A9 `cargo test -p clanky --lib recent_credits_agent` runs at least 1 test and passes: a command block run by
     the agent "rusty" is shown in Recent as rusty's, not the pane's.
On clanky-gpui (CLANKY_BIN): the servers and the windows are CLANKY_BIN (`--server --socket PATH` for the servers);
the `clanky agent` command line stays today's clanky, as it is not part of the window. A8 also reads the window's
control record (scripts/window-chrome/RECORD.md) after Ctrl+Shift+G: the screen "agent-picker" holds exactly the rows
"Fake" (enabled, checked: the first found tool is the highlighted one) and "Missing" (disabled: not installed), in
the config's order, and a popover object. In the Post-AI window no "agent-picker" screen line with controls is
written within 3 s. Each run's runtime folder is exactly 0700, and the window gets no desktop session variables.
"""
import base64, json, os, re, shutil, socket, subprocess, sys, time
from pathlib import Path
import importlib.util as _config_xml_util
_config_xml_spec = _config_xml_util.spec_from_file_location('clanky_config_xml', Path(__file__).with_name('clanky_config_xml.py'))
config_xml = _config_xml_util.module_from_spec(_config_xml_spec)
_config_xml_spec.loader.exec_module(config_xml)  # config.xml writer and reader shared by the gate harnesses

_gate_app_spec = _config_xml_util.spec_from_file_location('clanky_gate_app', Path(__file__).with_name('clanky_gate_app.py'))
gate_app = _config_xml_util.module_from_spec(_gate_app_spec)
_gate_app_spec.loader.exec_module(gate_app)  # which window binary the gate drives (CLANKY_BIN)

TARGET = Path(os.environ['CARGO_TARGET_DIR'])
APP = TARGET / 'debug/clanky'
GPUI = gate_app.override()
WINDOW_APP = GPUI or APP  # the servers and windows; the `clanky agent` command line is always today's clanky
PICKER_ROWS = [('Fake', True, True), ('Missing', False, False)]  # A8: (label, enabled, checked) in config order
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
ROOT = Path(os.environ.get('CLANKY_A1_WORK', '/srv/devops/build/clanky-a1-gate'))
FAKE = '#!/bin/sh\nprintf "FAKE-AGENT cwd=%s args=%s\\n" "$PWD" "$*"\n'
LIST_GATE = 'Fake\tfake-agent\tfound\nMissing\tno-such-agent-t9\tnot installed\n'
LIST_DEFAULT = 'Claude Code\tclaude\tfound\nCodex\tcodex\tnot installed\nPi\tpi\tnot installed\n'
AGENTS = [{'name': 'Fake', 'command': 'fake-agent'}, {'name': 'Missing', 'command': 'no-such-agent-t9'}]


class Fail(Exception):
    pass


def free_display():
    """A display number no X server uses: 100 and up, with no lock file and no socket file. Never :0: Marc's
    desktop Xwayland listens there under the socket name X0_, so Xvfb took :0, replaced /tmp/.X11-unix/X0 and
    deleted it on exit, cutting new X11 apps off his desktop (2026-11-13)."""
    taken = gate_app.abstract_x_displays()
    for n in range(100, 200):
        if not os.path.exists(f'/tmp/.X{n}-lock') and not os.path.lexists(f'/tmp/.X11-unix/X{n}') \
                and not os.path.lexists(f'/tmp/.X11-unix/X{n}_') and n not in taken:
            return str(n)
    raise Fail('no free X display between :100 and :199')


PROCS = []
SERVER_PIDS = []  # real server process IDs (a strace wrapper does not pass SIGTERM on)


def require(ok, label):
    if not ok:
        raise Fail(label)


def until(fn, label, timeout=10):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        v = fn()
        if v:
            return v
        time.sleep(.1)
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
        require(reply.get('ok') is True, f'RPC {op}: {reply.get("error")}')
        return reply['result']


def owner(sock):
    c = Conn(sock)
    # Authenticate first: a Post-AI server refuses every unauthenticated hello.
    c.call(3, 'auth', token=Path(str(sock) + '.owner').read_text().strip())
    pid = c.call(3, 'hello').get('server_pid')
    if pid and pid not in SERVER_PIDS:
        SERVER_PIDS.append(pid)
    return c


def tabs(o):
    return {t['tab_id']: t['allow_agent'] for t in o.call(1, 'list_tabs')['tabs']}


def text(o, tab):
    s = o.call(4, 'screen', tab_id=tab, history_offset=0, history_limit=64)
    lines = s.get('lines', []) + s.get('history', {}).get('lines', [])
    return '\n'.join(l if isinstance(l, str) else l.get('text', '') for l in lines)


def setup(name, stubs=('fake-agent',), **cfg):
    w = ROOT / name
    w.mkdir(mode=0o700)
    for d in ('home', 'config', 'data', 'state', 'runtime', 'bin', 'project'):
        (w / d).mkdir(mode=0o700)
        os.chmod(w / d, 0o700)  # no set-group-ID bit inherited from the build folder: the runtime folder is exactly 0700
    for stub in stubs:
        (w / 'bin' / stub).write_text(FAKE)
        (w / 'bin' / stub).chmod(0o755)
    env = {k: v for k, v in gate_app.isolated_env().items() if not k.startswith(('XDG_', 'CLANKY', 'WAYLAND', 'DBUS'))}
    env.update(HOME=str(w / 'home'), XDG_CONFIG_HOME=str(w / 'config'), XDG_DATA_HOME=str(w / 'data'),
               XDG_STATE_HOME=str(w / 'state'), XDG_RUNTIME_DIR=str(w / 'runtime'), SHELL='/bin/sh',
               PATH=f'{w / "bin"}:/usr/bin:/bin')
    config = w / 'config.xml'
    config_xml.write(config, dict(restore_layout=False, startup_animation=False, history=False, **cfg))
    return w, env, config


def server(w, env, config, trace=None):
    sock = w / 'server.sock'
    cmd = [str(WINDOW_APP), '--server', '--socket', str(sock), '--config', str(config)]
    if trace:
        cmd = ['strace', '-f', '-qq', '-e', 'trace=connect', '-o', str(trace)] + cmd
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    PROCS.append(p)
    until(lambda: sock.exists() or require(p.poll() is None, 'server startup'), 'socket startup', 20)
    o = owner(sock)
    o.call(1, 'open_tab', allow_agent=False)
    return sock, o


def agent(env, config, sock, *args, trace=None):
    cmd = [str(APP), 'agent', '--socket', str(sock), '--config', str(config), *map(str, args)]
    if trace and FAULT == 'network':
        # Planted: an outbound IPv4 connect inside the traced CLI process tree.
        probe = "import socket\ns=socket.socket()\ns.settimeout(.2)\ntry: s.connect(('127.0.0.1',9))\nexcept OSError: pass"
        cmd = ['sh', '-c', '"$@"; s=$?; python3 -c "$PROBE"; exit $s', 'x', *cmd]
        env = dict(env, PROBE=probe)
    if trace:
        cmd = ['strace', '-f', '-qq', '-e', 'trace=connect', '-o', str(trace)] + cmd
    return subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=30, cwd=str(config.parent))


def main_story():
    w, env, config = setup('main', agents=AGENTS)
    if FAULT == 'missing':
        (w / 'bin/fake-agent').unlink()
    sock, o = server(w, env, config, trace=w / 'server.strace')
    before = tabs(o)
    require(len(before) == 1 and list(before.values()) == [False], 'one owner terminal, agents off')
    r = agent(env, config, sock, 'fake', w / 'project', trace=w / 'cli.strace')
    require(r.returncode == 0, f'A1 exit code {r.returncode}')
    after = until(lambda: t if len(t := tabs(o)) == 2 else None, 'A1 one new terminal')
    new = [t for t in after if t not in before]
    require(len(new) == 1, 'A1 exactly one new terminal')
    tab = new[0]
    if FAULT == 'access':
        o.call(3, 'permission_set', tab_id=tab, allow_agent=False)
    require(after[tab] is True, 'A1 new terminal agent access on')
    require(tabs(o)[tab] is True, 'A1 new terminal agent access still on')
    require(all(after[t] is False for t in before), 'A1 other terminal stays off')
    until(lambda: f'FAKE-AGENT cwd={w / "project"} args=' in text(o, tab), 'A1 tool ran in DIR with no args')
    o.call(3, 'input', tab_id=tab, data=base64.b64encode(b'echo SHELL-$((40+2))\r').decode())
    until(lambda: 'SHELL-42' in text(o, tab), 'A2 shell still alive after the tool exits')
    r = agent(env, config, sock, 'missing')
    require(r.returncode == 1 and 'not installed' in r.stderr, 'A3 missing tool: exit 1, "not installed"')
    time.sleep(1)
    require(len(tabs(o)) == 2, 'A3 no terminal opened for a missing tool')
    r = subprocess.run([str(APP), 'agent', '--config', str(config), '--list'], env=env, capture_output=True,
                       text=True, timeout=30)
    require(r.returncode == 0 and r.stdout == LIST_GATE, 'A4 --list with the gate config')
    for name in ('server.strace', 'cli.strace'):
        t = (w / name).read_text() if (w / name).exists() else ''
        require((w / name).exists(), f'A7 {name} written')
        require(not re.search(r'connect\(\d+, \{sa_family=AF_INET6?,', t), f'A7 no network connect in {name}')


def defaults():
    w, env, config = setup('defaults', stubs=('claude',))
    r = subprocess.run([str(APP), 'agent', '--config', str(config), '--list'], env=env, capture_output=True,
                       text=True, timeout=30)
    require(r.returncode == 0 and r.stdout == LIST_DEFAULT, 'A4 --list defaults')


def access_off():
    w, env, config = setup('off', agents=AGENTS, agent_pane_access='off')
    sock, o = server(w, env, config)
    before = tabs(o)
    require(agent(env, config, sock, 'fake').returncode == 0, 'A5 exit code')
    after = until(lambda: t if len(t := tabs(o)) == 2 else None, 'A5 one new terminal')
    require([after[t] for t in after if t not in before] == [False], 'A5 access off when configured off')


def post_ai():
    w, env, config = setup('postai', agents=AGENTS, post_ai=(FAULT != 'post-ai'))
    sock, o = server(w, env, config)
    r = agent(env, config, sock, 'fake')
    require(r.returncode == 2, f'A6 Post-AI exit code {r.returncode}')
    time.sleep(1)
    require(len(tabs(o)) == 1, 'A6 no terminal in Post-AI mode')


def window(post):
    agents = AGENTS[::-1] if FAULT == 'picker' else AGENTS
    w, env, config = setup('window-postai' if post else 'window', agents=agents,
                           post_ai=post and FAULT != 'window-post-ai')
    rd, wr = os.pipe()
    x = subprocess.Popen(['Xvfb', ':' + free_display(), '-displayfd', str(wr), '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'],
                         pass_fds=(wr,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    PROCS.append(x)
    os.close(wr)
    env.update(DISPLAY=':' + os.read(rd, 32).decode().strip(), WINIT_UNIX_BACKEND='x11', LP_NUM_THREADS='8',
               VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json')
    os.close(rd)
    sock = w / 'window.sock'
    record = w / 'controls.jsonl'
    if GPUI:
        env['CLANKY_CONTROL_RECORD'] = str(record)
    p = subprocess.Popen([str(WINDOW_APP), '--socket', str(sock), '--config', str(config)], env=env,
                         cwd=str(w / 'project'), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    PROCS.append(p)
    until(lambda: sock.exists() or require(p.poll() is None, 'A8 window startup'), 'A8 socket', 30)
    # Only a mapped window takes the focus: clanky-gpui also owns an unmapped one, which xdotool lists first.
    wid = until(lambda: (subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(p.pid)], env=env, capture_output=True,
                                        text=True).stdout.split() or [None])[0], 'A8 window', 30)
    subprocess.run(['xdotool', 'windowfocus', '--sync', wid], env=env, check=True)
    o = owner(sock)
    before = until(lambda: tabs(o) or None, 'A8 first terminal', 20)
    time.sleep(1)
    subprocess.run(['xdotool', 'key', 'ctrl+shift+g'], env=env, check=True)
    if GPUI and post:
        time.sleep(3)
        require(picker_screen(record) is None, 'A8 Post-AI window draws no agent picker')
    elif GPUI:
        screen = until(lambda: picker_screen(record), 'A8 Ctrl+Shift+G draws the agent picker in the record', 5)
        rows = [(c['label'], c['enabled'], c['checked']) for c in screen['controls'] if c['kind'] == 'palette-row']
        require(rows == PICKER_ROWS and screen.get('popover'),
                f'A8 the picker lists {PICKER_ROWS}, Fake highlighted, in a popover (observed {rows})')
    time.sleep(0.8)
    subprocess.run(['xdotool', 'key', 'Return'], env=env, check=True)
    if post:
        time.sleep(3)
        require(len(tabs(o)) == len(before), 'A8 Post-AI window opens nothing')
        return
    after = until(lambda: t if len(t := tabs(o)) == len(before) + 1 else None, 'A8 shortcut opens one terminal')
    new = [t for t in after if t not in before][0]
    require(after[new] is True, 'A8 new terminal agent access on')
    until(lambda: 'FAKE-AGENT' in text(o, new), 'A8 tool ran in the new pane')


def picker_screen(record):
    """The agent picker's latest screen line in the control record while it lists controls, else None. A line the
    window is still writing is skipped."""
    text = record.read_text() if record.exists() else ''
    lines = text.splitlines()
    if lines and not text.endswith('\n'):
        lines = lines[:-1]
    screens = [json.loads(l) for l in lines if '"agent-picker"' in l]
    screens = [s for s in screens if s.get('record') == 'screen' and s.get('screen') == 'agent-picker']
    return screens[-1] if screens and screens[-1].get('controls') else None


def recent_test():
    # A build in the shared target folder: one at a time, under the build lock.
    lock = os.environ.get('CLANKY_BUILD_LOCK', '/srv/devops/build/clanky/.build.lock')
    r = subprocess.run(['flock', lock, 'cargo', 'test', '-q', '--offline', '-j', '8', '-p', 'clanky', '--lib', 'recent_credits_agent'],
                       capture_output=True, text=True, timeout=1800)
    m = re.search(r'test result: ok\. (\d+) passed', r.stdout)
    require(r.returncode == 0 and m and int(m.group(1)) >= 1, 'A9 recent_credits_agent test exists and passes')


def main():
    shutil.rmtree(ROOT, ignore_errors=True)
    ROOT.mkdir(parents=True, mode=0o700)
    steps = [('main story', main_story), ('defaults', defaults), ('access off', access_off), ('post-ai', post_ai)]
    if FAULT == 'picker':
        steps = [('window', lambda: window(False))]
    if FAULT == 'window-post-ai':
        steps = [('window post-ai', lambda: window(True))]
    if FAULT is None:
        steps += [('window', lambda: window(False)), ('window post-ai', lambda: window(True)),
                  ('recent', recent_test)]
    try:
        for name, step in steps:
            step()
            print(f'ok: {name}')
    except Fail as e:
        print(f'FAIL: A1 {e}')
        return 1
    except Exception as e:  # a crash in the harness or the product still reports FAIL, never PASS
        print(f'FAIL: A1 unexpected {type(e).__name__}: {e}')
        return 1
    finally:
        for pid in SERVER_PIDS:
            try:
                os.kill(pid, 15)
            except ProcessLookupError:
                pass
        for p in PROCS:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    p.kill()
        if FAULT is None:
            shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: A1 agent pane')
    return 0


if __name__ == '__main__':
    sys.exit(main())
