#!/usr/bin/env python3
"""Remaining check-8 controls through real X11 input, config files and pixels.
All expected points/colours are fixed in docs/m12-click-checks.md before the run.

Quiet layout (design 2026-11-13, Marc's Y1 of 2026-11-14): Settings opens from the title bar's
gear (1217,19) [mockup .titlebar: padding 10, .ico 26; Q1 gate T3]; its frame, nav and pages keep their places
[Q1]. Appearance's theme cards and Light/System/Dark segments are replaced by the theme picker [Q1 K1-K3, C1]: a
Dark list in the left column (Graphite, Ink, Stone, ...) and a Light list in the right one (Graphite, Stone, ...); a
click on a row saves it. Stone dark is found by its word in the left half of the page body (scrolling the page when
needed), as the C1 gate tells the two lists apart; "Follow the system" On
writes theme "system", Off writes "dark" or "light" (the system's mode). The Terminal page's Side columns row is
now the Tools panel row at the same place: it writes tools_panel "off"/"on" and no "rails" [Q1]. The sidebar
toggle is the title bar's tools toggle (23,19) [mockup .ico at x 10..36; Q1 T3].

Settings with every setting (C1): the nav lists General, Appearance, Terminal, Editor, Agents, History, Keys, and
the rows moved, so no Settings control is clicked at a fixed point any more. Settings opens with Ctrl+, and the nav
and every row are found by their words, read more than one way, with the C1 gate's helpers
(scripts/gate-c1-config.py: open_settings, page, label_at, row_band, rightmost_mark, readings). Font size is a field
on Appearance (the old stepper's minus is typed: 14 -> 13), the Tools panel row is Appearance's On | Off segment pair,
and the Post-AI switch is the mark at the right of its label's row. Every expected colour is computed from
assets/themes/*.json (the active theme's tokens, an alpha colour over its background), never read from the build.
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


def load_c1(work):
    """The C1 gate's Settings helpers, with their OCR scratch file in this run's folder."""
    os.environ['CLANKY_C1_WORK'] = str(work)
    argv = sys.argv
    sys.argv = argv[:1]
    try:
        spec = _config_xml_util.spec_from_file_location('c1', Path(__file__).with_name('gate-c1-config.py'))
        module = _config_xml_util.module_from_spec(spec)
        spec.loader.exec_module(module)
    finally:
        sys.argv = argv
    return module


def theme_colour(doc, token):
    """A theme token as RGB bytes; #rrggbbaa is composed over the theme's background."""
    def rgb(h):
        return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))
    value = doc['colors'][token]
    if len(value) == 9:
        alpha = int(value[7:9], 16) / 255
        return tuple(round(c * alpha + b * (1 - alpha)) for c, b in zip(rgb(value), rgb(doc['colors']['background'])))
    return rgb(value)


def count_near(im, box, colour, tol=3):
    import numpy as np
    a = np.asarray(im.crop(tuple(int(v) for v in box))).astype(int)
    return int((np.abs(a - np.array(colour)).max(axis=2) <= tol).sum())


def main(work, bins):
    root = Path(__file__).resolve().parent.parent
    c1 = load_c1(work)
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
    stone_dark = json.loads((root / 'assets/themes/stone-dark.json').read_text())
    stone_light = json.loads((root / 'assets/themes/stone-light.json').read_text())
    fixture = json.loads(json.dumps(stone_dark))
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

        class SettingsWindow:
            """What the C1 gate's helpers need of a window: shots, real clicks, keys and the Settings boxes."""
            frame = nav = content = None

            def __init__(self):
                self.wid, self.config, self.p, self.pid0 = wid, config, app, app.pid

            def shot(self, label):
                return capture(label)

            def click(self, x, y):
                click(int(x), int(y))

            def x(self, *args):
                xdo(*args)

            def key(self, *keys):
                for k in keys:
                    xdo('key', k)
                    time.sleep(.3)

            def type(self, text):
                xdo('type', '--delay', '25', text)

            def alive(self):
                return app.poll() is None

        win = SettingsWindow()
        names = ('General', 'Appearance', 'Terminal', 'Editor', 'Agents', 'History', 'Keys')
        theme = dict(doc=fixture)  # the active theme document; its tokens give every expected colour

        def section(name):
            c1.page(win, name)
            image = capture('section-' + name.lower())
            # A point inside the nav rows, right of the longest name, on each name's line.
            x = int(max(w[3] for w in win.nav.values())) + 12
            on, off = theme_colour(theme['doc'], 'sidebar_accent'), theme_colour(theme['doc'], 'sidebar')
            for other in names:
                w = win.nav[other.lower()]
                pixel(image, (x, int((w[2] + w[4]) / 2)), on if other == name else off, name + ' selected section')
            results.append('section ' + name)
            return image

        # Opening/closing is setup: clanky-pkg owns those acceptance checks.
        c1.open_settings(win, 'Settings opens with its seven groups')
        section('Appearance')
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

        # The picker shows the dark list in the left column and the light list in the right one [C1: "Mocha" in the
        # dark list, "Paper" in the light list; Q1: dark on the left], so Stone dark is the Stone left of the body's
        # middle (the light list's Stone is its second row and would be found first).
        middle = (body[0] + body[2]) / 2
        stone, _ = find('stone-row', lambda words: min((w for w in words if w[0] == 'stone' and w[2] < 600
                                                        and (w[1] + w[3]) / 2 < middle),
                                                       key=lambda w: w[2], default=None))
        if fault != 'click':
            click(*centre(stone))  # Stone in the Dark column: theme_dark stone-dark
        image = capture('stone-row')
        theme['doc'] = stone_dark
        pixel(image, (997, 600), theme_colour(stone_dark, 'background'), 'theme row applies Stone pixel')
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
        theme['doc'] = stone_dark if mode == 'dark' else stone_light
        pixel(capture('follow-off'), (997, 600), theme_colour(theme['doc'], 'background'),
              'Follow the system Off background')
        results.append('Follow the system On and Off')

        section('General')
        lab, image = c1.label_at(win, 'Post-AI mode')
        band = c1.row_band(win, lab)
        mark = c1.rightmost_mark(image, int(lab[2]) + 40, band)
        require(mark, 'the "Post-AI mode" row has a switch')
        # The switch: a primary track when on; a muted_foreground knob and no primary ink when off.
        knob = (mark[0] - 40, band[1], min(win.content[2], mark[0] + 12), band[3])
        primary, knob_off = theme_colour(theme['doc'], 'primary'), theme_colour(theme['doc'], 'muted_foreground')
        click(*mark)
        setting('post_ai', True)
        image = capture('post-ai-on')
        require(count_near(image, knob, primary) >= 60,
                f'Post-AI switch turns on: primary {primary} track ({count_near(image, knob, primary)} px)')
        click(*mark)
        setting('post_ai', False)
        image = capture('post-ai-off')
        require(count_near(image, knob, primary) < 5 and count_near(image, knob, knob_off) >= 30,
                f'Post-AI switch turns off: no primary track, a {knob_off} knob '
                f'({count_near(image, knob, primary)} / {count_near(image, knob, knob_off)} px)')
        results.append('Post-AI switch')

        section('Appearance')
        lab, image = c1.label_at(win, 'Font size')
        hit = None
        for words in c1.band_words(win, image, c1.row_band(win, lab)):
            hit = next((w for w in words if w[1] > lab[2] + 6 and c1.num_is(14)(w[0])), None)
            if hit:
                break
        require(hit, 'the "Font size" field shows 14')
        click(*centre(hit))
        win.key('ctrl+a')
        win.type('13')
        win.key('Return')
        setting('font_size', 13)
        results.append('Font size one step down, 14 to 13')

        def tools_segment(word):
            lab, image = c1.label_at(win, 'Tools panel')
            for words in c1.band_words(win, image, c1.row_band(win, lab)):
                hits = [w for w in words if w[1] > lab[2] + 8 and c1.norm(w[0]) == word]
                if hits:
                    return centre(max(hits, key=lambda w: w[1]))
            raise Failure(f'the "Tools panel" row has an "{word}" segment')

        click(*tools_segment('off'))
        setting('tools_panel', 'off')
        require('rails' not in saved(), 'Tools panel row removes rails')
        pixel(capture('columns-off'), (128, 700), theme_colour(theme['doc'], 'background'), 'Tools panel Off hides the panel')
        click(*tools_segment('on'))
        # "on" is the default, which Settings may write by removing the key [Q1].
        until(lambda: saved().get('tools_panel', 'on') == 'on' and 'rails' not in saved(), 'saved tools_panel on')
        pixel(capture('columns-on'), (128, 700), theme_colour(theme['doc'], 'sidebar'), 'Tools panel On restores the panel')
        results.append('Tools panel On and Off')

        for name in ('Terminal', 'Editor', 'History'):
            section(name)
        section('Agents')

        def word_box(word):
            def found(im):
                for words in c1.readings(im, win.content):
                    w = next((w for w in words if c1.token_ok(w[0], word)), None)
                    if w:
                        return w[1:]
            return c1.seek(win, found, f'"{word}" on the page')

        box, image = word_box('remove')
        border = theme_colour(theme['doc'], 'input')
        around = (box[0] - 16, box[1] - 12, box[2] + 16, box[3] + 12)
        require(count_near(image, around, border) >= 20,
                f'Agents page Remove button border {border} ({count_near(image, around, border)} px)')
        section('Keys')
        box, image = word_box('ctrl')
        chip = theme_colour(theme['doc'], 'muted')
        around = (box[0] - 6, box[1] - 6, box[2] + 6, box[3] + 6)
        require(count_near(image, around, chip) >= 20,
                f'Keys page shortcut chip {chip} ({count_near(image, around, chip)} px)')
        xdo('key', '--window', wid, 'Escape')
        settle()
        click(23, 19)
        pixel(capture('sidebar-hidden'), (128, 700), theme_colour(theme['doc'], 'background'), 'tools toggle hides the panel')
        click(23, 19)
        pixel(capture('sidebar-shown'), (128, 700), theme_colour(theme['doc'], 'sidebar'), 'tools toggle shows the panel')
        results.append('tools toggle')

        stop(app)
        for entry in call('list_tabs')['tabs']:
            call('close_tab', dict(tab_id=entry['tab_id']))
        (pictures / 'result.json').write_text(json.dumps(dict(passed=results), indent=2) + '\n')
        print('PASS: remaining real-window Settings clicks (seven sections, Follow the system, theme row pixel, font size step, Post-AI switch, Tools panel, Agents and Keys pages, tools toggle)')
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
    except BaseException as error:
        if type(error).__name__ in ('Failure', 'Fail'):  # Fail: raised by the C1 gate's helpers
            print('FAIL: M12 Settings clicks ' + str(error), file=sys.stderr)
        else:
            print(f'FAIL: M12 Settings clicks fixture or transport ({type(error).__name__}: {error})', file=sys.stderr)
        sys.exit(1)
