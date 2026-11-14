#!/usr/bin/env python3
"""Remaining check-8 controls through real X11 input, config files and pixels.
All expected points/colours are fixed in docs/m12-click-checks.md before the run.

Quiet layout (design 2026-11-13, Marc's Y1 of 2026-11-14): Settings opens from the title bar's
gear (1217,19) [mockup .titlebar: padding 10, .ico 26; Q1 gate T3]; its frame, nav and pages keep their places
[Q1]. Appearance's theme cards and Light/System/Dark segments are replaced by the theme picker [Q1 K1-K3]: with
Follow the system off, one list (dark themes first: Graphite, Ink, Stone, ...); a click on a row saves it. The
Stone row is found by its word (scrolling the page when needed), as the Q1 gate does; "Follow the system" On
writes theme "system", Off writes "dark" or "light" (the system's mode). The Terminal page's Side columns row is
now the Tools panel row at the same place: it writes tools_panel "off"/"on" and no "rails" [Q1]. The sidebar
toggle is the title bar's tools toggle (23,19) [mockup .ico at x 10..36; Q1 T3].
"""
import json
import os
from pathlib import Path
import importlib.util as _config_xml_util
_config_xml_spec = _config_xml_util.spec_from_file_location('clanky_config_xml', Path(__file__).with_name('clanky_config_xml.py'))
config_xml = _config_xml_util.module_from_spec(_config_xml_spec)
_config_xml_spec.loader.exec_module(config_xml)  # config.xml writer and reader shared by the gate harnesses
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


def ocr_words(im, box, tmp, scale=3, psm=11):
    """Words in box as (text, x0, y0, x1, y1) in window pixels, lowercased (the Q1 gate's reader)."""
    from PIL import Image, ImageOps
    crop = im.crop(box).convert('L')
    if sum(crop.getdata()) / max(1, crop.width * crop.height) < 128:
        crop = ImageOps.invert(crop)
    crop = crop.resize((crop.width * scale, crop.height * scale), Image.LANCZOS)
    path = Path(tmp) / 'ocr.png'
    crop.save(path)
    out = subprocess.run(['tesseract', str(path), '-', '--psm', str(psm), 'tsv'], capture_output=True, text=True,
                         timeout=60).stdout
    words = []
    for line in out.splitlines()[1:]:
        f = line.split('\t')
        if len(f) == 12 and f[11].strip():
            x, y, w, h = (int(v) for v in f[6:10])
            words.append((f[11].strip().lower(), box[0] + x / scale, box[1] + y / scale,
                          box[0] + (x + w) / scale, box[1] + (y + h) / scale))
    return words


def centre(w):
    return int((w[1] + w[3]) / 2), int((w[2] + w[4]) / 2)


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
    config = work / 'config.xml'
    config_xml.write(config, dict(theme='dark', theme_dark='user:click-fixture', theme_light='stone-light',
                                  font_family='Geist Mono', ui_font_family='Geist', font_size=14,
                                  columns=155, rows=37, opacity=1, rails=True, animations=False,
                                  startup_animation=False, restore_layout=False, post_ai=False,
                                  osc52_write=False, osc52_read=False))
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
        return config_xml.load(config)

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
        click(1217, 19)  # the title bar's settings gear
        section(1)
        pixel(capture('before-theme'), (997, 600), (1, 2, 3), 'synthetic initial background')
        body = (459, 130, 985, 630)  # Settings body text, the Q1 gate's OCR box moved to the 1240 x 760 frame

        def find(label, test, notches=30):
            for _ in range(notches):
                words = ocr_words(capture(label), body, work)
                hit = test(words)
                if hit:
                    return hit, words
                xdo('mousemove', '--window', wid, 640, 380)
                xdo('click', '5')
                settle()
            raise Failure(f'"{label}" found on the Appearance page')

        stone, _ = find('stone-row', lambda words: min((w for w in words if w[0] == 'stone' and w[2] < 600),
                                                     key=lambda w: w[2], default=None))
        if fault != 'click':
            click(*centre(stone))  # the first Stone row: Stone dark, as the picker lists dark themes first
        image = capture('stone-row')
        pixel(image, (997, 600), (20, 18, 16), 'theme row applies Stone pixel')
        setting('theme_dark', 'stone-dark')
        results.append('theme row pixel')

        def follow_segment(name):
            def test(words):
                follow = min((w for w in words if w[0] == 'follow'), key=lambda w: w[2], default=None)
                if not follow:
                    return None
                return max((w for w in words if w[0] == name and abs(w[2] - follow[2]) < 30), key=lambda w: w[1],
                           default=None)
            return find('follow-' + name, test)[0]

        click(*centre(follow_segment('on')))
        setting('theme', 'system')
        click(*centre(follow_segment('off')))
        mode = until(lambda: saved().get('theme') if saved().get('theme') in ('dark', 'light') else None,
                     'saved theme dark or light')
        pixel(capture('follow-off'), (997, 600), (20, 18, 16) if mode == 'dark' else (251, 250, 248),
              'Follow the system Off background')
        results.append('Follow the system On and Off')

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
        setting('tools_panel', 'off')
        require('rails' not in saved(), 'Tools panel row removes rails')
        pixel(capture('columns-off'), (128, 700), (20, 18, 16), 'Tools panel label hides the panel')
        click(500, 450)
        # "on" is the default, which Settings may write by removing the key [Q1].
        until(lambda: saved().get('tools_panel', 'on') == 'on' and 'rails' not in saved(), 'saved tools_panel on')
        pixel(capture('columns-on'), (128, 700), (15, 13, 11), 'Tools panel label restores the panel')
        results.append('Tools panel row label')

        image = section(3)
        pixel(image, (980, 250), (49, 45, 41), 'Agents page stop button border')
        image = section(4)
        pixel(image, (975, 172), (31, 28, 25), 'Keys page shortcut chip')
        xdo('key', '--window', wid, 'Escape')
        settle()
        click(23, 19)
        pixel(capture('sidebar-hidden'), (128, 700), (20, 18, 16), 'tools toggle hides the panel')
        click(23, 19)
        pixel(capture('sidebar-shown'), (128, 700), (15, 13, 11), 'tools toggle shows the panel')
        results.append('tools toggle')

        stop(app)
        for entry in call('list_tabs')['tabs']:
            call('close_tab', dict(tab_id=entry['tab_id']))
        (pictures / 'result.json').write_text(json.dumps(dict(passed=results), indent=2) + '\n')
        print('PASS: remaining real-window Settings clicks (sections, Follow the system, theme row pixel, minus, row labels, tools toggle)')
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
