#!/usr/bin/env python3
"""Remaining check-8 controls through real X11 input, config files and pixels.
All expected points/colours are fixed in docs/m12-click-checks.md before the run.
"""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time
from PIL import ImageGrab


class Failure(Exception):
    pass


def require(ok, label):
    if not ok:
        raise Failure(label)


def until(fn, label, seconds=10):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        result = fn()
        if result:
            return result
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


def main(work, bins):
    root = Path(__file__).resolve().parent.parent
    target = Path(os.environ['CARGO_TARGET_DIR'])
    pictures = target / 'look/settings-clicks'
    pictures.mkdir(parents=True, exist_ok=True)
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'click'), 'unknown Settings click fault')
    env = {k: v for k, v in os.environ.items()
           if not any(word in k.upper() for word in ('KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD'))}
    for key in ('WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'CLANKY_ROBOT_GATE',
                'CLANKY_ROBOT_CLOCK', 'CLANKY_FRAME_TRACE'):
        env.pop(key, None)
    for name in ('home', 'config', 'state', 'runtime'):
        (work / name).mkdir(mode=0o700)
    env.update(HOME=str(work / 'home'), XDG_CONFIG_HOME=str(work / 'config'),
               XDG_STATE_HOME=str(work / 'state'), XDG_RUNTIME_DIR=str(work / 'runtime'),
               SHELL='/bin/sh', WINIT_UNIX_BACKEND='x11')
    themes = work / 'config/clanky/themes'
    themes.mkdir(parents=True)
    fixture = json.loads((root / 'assets/themes/stone-dark.json').read_text())
    fixture['name'] = 'Click fixture'
    fixture['colors']['background'] = '#010203'
    (themes / 'click-fixture.json').write_text(json.dumps(fixture))
    config = work / 'config.json'
    config.write_text(json.dumps(dict(theme='dark', theme_dark='user:click-fixture', theme_light='stone-light',
                                     font_family='Geist Mono', ui_font_family='Geist', font_size=14,
                                     columns=155, rows=37, opacity=1, rails=True, animations=False,
                                     startup_animation=False, restore_layout=False, post_ai=False,
                                     osc52_write=False, osc52_read=False)))
    socket = work / 'server.sock'
    trace = work / 'frames.csv'
    children, logs = [], []
    results = []

    def launch(argv, child_env, label, pipe=False):
        log = (work / (label + '.log')).open('wb')
        logs.append(log)
        process = subprocess.Popen([str(x) for x in argv], cwd=work, env=child_env,
                                   stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE if pipe else subprocess.DEVNULL, stderr=log)
        children.append(process)
        return process

    def xdo(*args):
        result = subprocess.run(['xdotool', *map(str, args)], env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=10)
        require(result.returncode == 0, f'xdotool {args[0]} exit {result.returncode}: {result.stderr.decode(errors="replace").strip()}')
        return result.stdout.decode().strip()

    def frames():
        return sum(line.startswith('frame,') for line in trace.read_text().splitlines()) if trace.exists() else 0

    def saved():
        return json.loads(config.read_text())

    try:
        launch([bins / 'clanky', '--server', '--socket', socket], env, 'server')
        until(lambda: socket.exists(), 'private server startup')
        owner = launch([bins / 'clanky-agent-gate', 'owner', socket], env, 'owner', pipe=True)

        def call(op, args=None):
            owner.stdin.write(json.dumps(dict(v=1, op=op, args=args or {})).encode() + b'\n')
            owner.stdin.flush()
            require(select.select([owner.stdout], [], [], 10)[0], 'owner reply deadline')
            reply = json.loads(owner.stdout.readline())
            require(reply.get('ok'), 'owner RPC ' + op)
            return reply['result']

        tab = call('open_tab', dict(rows=24, cols=80, allow_agent=False))['tab_id']
        call('run', dict(tab_id=tab, argv=['sh', '-c', 'sleep 600'], cwd=str(work)))
        app = launch([bins / 'clanky', '--socket', socket, '--config', config],
                     dict(env, CLANKY_FRAME_TRACE=str(trace)), 'window')
        until(lambda: frames() > 0, 'initial window frame')
        wid = xdo('search', '--onlyvisible', '--pid', app.pid).splitlines()[0]
        xdo('windowsize', wid, 1240, 760)
        xdo('windowmove', wid, 0, 0)
        xdo('windowfocus', '--sync', wid)
        time.sleep(.4)

        def settle():
            count, since, deadline = frames(), time.monotonic(), time.monotonic() + 5
            while time.monotonic() - since < .12:
                require(time.monotonic() < deadline, 'pointer frame settles')
                require(app.poll() is None, 'window remains alive')
                time.sleep(.01)
                current = frames()
                if current != count:
                    count, since = current, time.monotonic()

        def click(x, y):
            before = frames()
            xdo('mousemove', '--window', wid, x, y)
            xdo('click', '1')
            # Remove hover from sampled controls, without sending another click.
            xdo('mousemove', '--window', wid, 1100, 730)
            until(lambda: frames() > before, 'pointer event redraw')
            settle()

        def capture(label):
            settle()
            geometry = dict(line.split('=', 1) for line in xdo('getwindowgeometry', '--shell', wid).splitlines())
            x, y, w, h = (int(geometry[k]) for k in ('X', 'Y', 'WIDTH', 'HEIGHT'))
            require((w, h) == (1240, 760), 'fixed click window size')
            image = ImageGrab.grab(bbox=(x, y, x + w, y + h), xdisplay=env['DISPLAY']).convert('RGB')
            image.save(pictures / (label + '.png'))
            return image

        def pixel(image, point, expected, label):
            observed = image.getpixel(point)
            require(all(abs(a - b) <= 3 for a, b in zip(observed, expected)),
                    f'{label}: expected {expected}, observed {observed}')

        def setting(key, value):
            until(lambda: saved().get(key) == value, 'saved ' + key)

        names = ('General', 'Appearance', 'Terminal', 'Agents', 'Keys')

        def section(index):
            click(330, 154 + 34 * index)
            image = capture('section-' + names[index].lower())
            for i in range(5):
                pixel(image, (410, 154 + 34 * i), (26, 24, 21) if i == index else (15, 13, 11),
                      names[index] + ' selected section')
            results.append('section ' + names[index])
            return image

        # Opening/closing is setup: clanky-pkg owns those acceptance checks.
        click(80, 680)
        section(1)
        pixel(capture('before-theme'), (997, 600), (1, 2, 3), 'synthetic initial background')
        if fault != 'click':
            click(520, 300)  # First card is Stone, as in the approved sheet.
        image = capture('stone-card')
        pixel(image, (997, 600), (20, 18, 16), 'theme card applies Stone pixel')
        setting('theme_dark', 'stone-dark')
        results.append('theme card pixel')

        click(943, 179)
        setting('theme', 'system')
        click(880, 179)
        setting('theme', 'dark')
        pixel(capture('dark-mode'), (997, 600), (20, 18, 16), 'Dark segment background')
        results.append('System and Dark segments')

        section(0)
        click(500, 175)
        setting('post_ai', True)
        pixel(capture('post-ai-on'), (955, 197), (235, 230, 223), 'Post-AI label toggles on')
        click(500, 175)
        setting('post_ai', False)
        pixel(capture('post-ai-off'), (955, 197), (20, 18, 16), 'Post-AI label toggles off')
        results.append('Post-AI row label')

        section(2)
        click(888, 288)
        setting('font_size', 13)
        results.append('stepper minus 14 to 13')
        click(500, 450)
        setting('rails', False)
        pixel(capture('columns-off'), (128, 700), (20, 18, 16), 'Side columns label hides columns')
        click(500, 450)
        setting('rails', True)
        pixel(capture('columns-on'), (128, 700), (20, 18, 16), 'Side columns label restores columns')
        results.append('Side columns row label')

        image = section(3)
        pixel(image, (980, 250), (54, 50, 47), 'Agents page stop button border')
        image = section(4)
        pixel(image, (975, 172), (31, 28, 25), 'Keys page shortcut chip')
        xdo('key', '--window', wid, 'Escape')
        settle()
        click(288, 36)
        pixel(capture('sidebar-hidden'), (128, 700), (20, 18, 16), 'sidebar toggle hides columns')
        click(40, 36)
        pixel(capture('sidebar-shown'), (128, 700), (20, 18, 16), 'sidebar toggle shows columns')
        results.append('sidebar toggle')

        stop(app)
        for entry in call('list_tabs')['tabs']:
            call('close_tab', dict(tab_id=entry['tab_id']))
        (pictures / 'result.json').write_text(json.dumps(dict(passed=results), indent=2) + '\n')
        print('PASS: remaining real-window Settings clicks (sections, modes, theme pixel, minus, row labels, sidebar)')
    finally:
        for process in reversed(children):
            stop(process)
        for log in logs:
            log.close()
        if trace.exists():
            (pictures / 'frames.csv').write_bytes(trace.read_bytes())


if __name__ == '__main__':
    try:
        main(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
    except Failure as error:
        print('FAIL: M12 Settings clicks ' + str(error), file=sys.stderr)
        sys.exit(1)
    except BaseException:
        print('FAIL: M12 Settings clicks fixture or transport', file=sys.stderr)
        sys.exit(1)
