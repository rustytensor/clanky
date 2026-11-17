"""The pane part of the robot startup and robot window checks, for a one-pane window without the startup robot
(CLANKY_BIN, such as clanky-gpui): a private server, home and state; real X11 input; the window's own frame log.

  first frame   the window logs a drawn frame within 15 s of being started
  idle          0 frames in the 2 s after it settles (cursor blink and animations off)
  key           xdotool's x reaches the pane's program as exactly the byte 'x', and the window draws its echo
  pixels        (with pixels=True) the X server's pixels inside the window hold more than one colour

The robot greeting, its timing and the New tab button are left out: that window has none of them yet.
"""

import json
import select
import subprocess
import sys
import time
import importlib.util
from pathlib import Path


def _load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(f'{name}.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


config_xml = _load('clanky_config_xml')
app = _load('clanky_gate_app')

READER = ("import os,tty\ntty.setraw(0)\nos.write(1,b'READY\\r\\n')\nopen('reader-ready','w').close()\n"
          "b=os.read(0,1)\nopen('received','wb').write(b)\nos.write(1,b'ECHO:'+b+b'\\r\\n')\n"
          "while not os.path.exists('release'):\n import time; time.sleep(.05)\n")


class Failure(Exception):
    pass


def require(ok, label):
    if not ok:
        raise Failure(label)


def until(fn, label, seconds=15):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = fn()
        if value:
            return value
        time.sleep(.01)
    raise Failure(label)


def stop(process):
    if process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=4)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=4)


def frames(trace):
    if not trace.exists():
        return 0
    return sum(line.startswith('frame,') for line in trace.read_text().splitlines())


def check(work, bins, pixels=False):
    """Runs the pane part; returns a one-line summary or raises Failure."""
    env = app.isolated_env()
    env.pop('CLANKY_FRAME_TRACE', None)
    for name in ('home', 'config', 'state', 'runtime', 'data', 'cache'):
        (work / name).mkdir(mode=0o700)
    env.update(HOME=str(work / 'home'), XDG_CONFIG_HOME=str(work / 'config'), XDG_STATE_HOME=str(work / 'state'),
               XDG_RUNTIME_DIR=str(work / 'runtime'), XDG_DATA_HOME=str(work / 'data'),
               XDG_CACHE_HOME=str(work / 'cache'), SHELL='/bin/sh')
    require(env.get('DISPLAY'), 'a private X display (run under xvfb-run)')
    config = work / 'config.xml'
    config_xml.write(config, dict(columns=124, rows=37, theme='dark', restore_layout=False, animations=False,
                                  startup_animation=False, cursor_blink='off', osc52_write=False, osc52_read=False))
    (work / 'reader.py').write_text(READER)
    socket = work / 'server.sock'
    trace = work / 'frames.csv'
    children, logs = [], []

    def launch(argv, child_env, label, pipe=False):
        log = (work / f'{label}.log').open('wb')
        logs.append(log)
        process = subprocess.Popen([str(x) for x in argv], env=child_env, cwd=work, stderr=log,
                                   stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE if pipe else subprocess.DEVNULL)
        children.append(process)
        return process

    def xdo(*args):
        result = subprocess.run(['xdotool', *map(str, args)], env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=10)
        require(result.returncode == 0, f'X11 input command {args[0]}')
        return result.stdout.decode().strip()

    try:
        launch(app.server_argv(bins, socket), env, 'server')
        until(socket.exists, 'private server startup')
        owner = launch([app.agent_gate(bins), 'owner', socket], env, 'owner', pipe=True)

        def call(op, args=None):
            owner.stdin.write(json.dumps(dict(v=1, op=op, args=args or {})).encode() + b'\n')
            owner.stdin.flush()
            require(select.select([owner.stdout], [], [], 15)[0], f'owner reply to {op}')
            reply = json.loads(owner.stdout.readline())
            require(reply.get('ok'), f'owner RPC {op}')
            return reply['result']

        tab = call('open_tab', dict(rows=24, cols=80, allow_agent=False))['tab_id']
        call('run', dict(tab_id=tab, argv=[sys.executable, str(work / 'reader.py')], cwd=str(work)))
        until((work / 'reader-ready').exists, 'the pane program started')
        started = time.monotonic()
        window = launch([app.window_binary(bins), '--socket', socket, '--config', config],
                        dict(env, CLANKY_FRAME_TRACE=str(trace)), 'window')
        until(lambda: frames(trace) > 0 or (window.poll() is not None and require(False, 'window startup')),
              'first frame', 15)
        first_ms = (time.monotonic() - started) * 1000
        wid = until(lambda: (xdo('search', '--onlyvisible', '--pid', window.pid).split() or [None])[0], 'window id')
        xdo('windowfocus', '--sync', wid)
        time.sleep(.5)
        before = frames(trace)
        time.sleep(2)
        idle = frames(trace) - before
        require(idle == 0, f'idle: expected 0 frames in 2 s, observed {idle}')
        colours = None
        if pixels:
            from PIL import ImageGrab
            geometry = xdo('getwindowgeometry', '--shell', wid)
            g = dict(line.split('=') for line in geometry.splitlines())
            x, y, w, h = (int(g[k]) for k in ('X', 'Y', 'WIDTH', 'HEIGHT'))
            image = ImageGrab.grab(bbox=(x, y, x + w, y + h), xdisplay=env['DISPLAY'])
            colours = len(image.getcolors(1 << 24) or [])
            require(colours > 1, f'pixels: the window shows {colours} colour(s)')
        before = frames(trace)
        xdo('key', '--window', wid, 'x')
        until(lambda: (work / 'received').exists() and (work / 'received').read_bytes(), 'key reaches the pane')
        require((work / 'received').read_bytes() == b'x', 'the exact key byte x')
        until(lambda: frames(trace) > before, 'the echo is drawn', 5)
        (work / 'release').write_text('')
        call('close_tab', dict(tab_id=tab))
        summary = f'first frame {first_ms:.0f} ms, 0 idle frames, key x reached the pane and was drawn'
        return summary + (f', {colours} colours on screen' if pixels else '')
    finally:
        for process in reversed(children):
            stop(process)
        for log in logs:
            log.close()
