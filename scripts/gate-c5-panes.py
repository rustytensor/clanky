#!/usr/bin/env python3
"""C5 gate: "New windows open with" One pane | Two panes (Marc 2026-11-15: a setting with default Two panes, today's
behaviour of a new window: one tab split in two, one pane above the other).

Usage: gate-c5-panes.py [--fault nokey|noclick|badvalue|norestore|idle]
A fault changes the run's input on purpose; a correct build must then FAIL, at the check the fault names.

Harness: the C1 gate's window and Settings helpers (scripts/gate-c1-config.py, itself the Q1 and T2 harness: a
private Xvfb display from :100 up, 1400 x 900 px at scale 1, software Vulkan, TZ=UTC, SHELL=/bin/sh, the owner API,
the frame trace, tesseract read more than one way), except that the window keeps the panes it opens with (the older
harnesses close the second startup pane). Each window has its own HOME, XDG folders and socket, so each one starts
its own fresh server; config.xml is written by scripts/clanky_config_xml.py and passed with --config. Clicks and keys
are real X input (xdotool). config.xml is read with Python's xml.etree.ElementTree and a regular expression on the
raw text, never by Clanky.

Interface fixed by this gate.
  Key new_window_panes, values 1 or 2, default 2: <new_window_panes>1</new_window_panes> in config.xml. A bad value
  (anything else) falls back to 2 and is reported like every other bad value: the window opens, Settings shows the
  error with its line ("line N") and names the key, and the file is not rewritten until the user changes something.
  Settings > General has a row labelled "New windows open with" with the segments "One pane" | "Two panes"; a click
  on one saves it at once (Two panes may also remove the element, being the default). The row says that it applies
  later (its text holds "Applies" and "window" besides the label). The open window keeps its panes; the next new
  window follows the setting. A restored session layout (restore_layout true and a saved layout.json, a fresh
  server) wins over the setting.

Pane count, two independent ways: the owner API (v3 workspace_get: the leaves of the active group's layout; v1
list_tabs: the terminals) and the pixels: the window's terminal background is set to PANE_BG #1b2a3a (not a colour
of the window chrome), and a pane is a 4-connected region of pixels within 16 of it, at least 200 x 100 px, that
fills at least 60 % of its bounding box. The pane's rectangle is that bounding box.

Sources. [M] Marc 2026-11-15: values 1 | 2, default 2, a bad value falls back to 2 and shows the error like every
other key, the Settings > General row, applies to the next new window and says so, a restored layout wins. [R1]
Marc's R1 2026-11-14: 8 px gap between panes, 14 px padding inside each pane. [S] docs/settings.md: the error banner
"config.xml line N: ...". [V] xterm: ESC[?1049h alternate screen, ESC[r;cH cursor position, ESC[48;2;r;g;bm truecolour
background.

Expected results, fixed before the work (window 1400 x 900, scale 1, default theme, terminal background PANE_BG):
  P1 a fresh window, no new_window_panes key [M]: 2 panes by the owner API (2 leaves, 2 terminals) and 2 pane
     regions in the pixels, one above the other (the same x range +-1); between them a gap of 8 rows [R1] (+-1).
     Their union (the top pane's top to the bottom pane's bottom) is the pane area AREA.
  I1 0 frames from 1 s to 3.5 s once that window settles.
  P2 <new_window_panes>1</new_window_panes> [M]: 1 pane by the owner API and 1 pane region whose rectangle equals
     AREA (each edge +-1): one pane fills the pane area. A program on the alternate screen [V] paints magenta cells
     at (1,1) and at the last column of the last row: cell (1,1) starts 14 right of the pane's left edge and 14 below
     its top [R1]; right of the last cell the pane goes on for 14 .. 14 + one cell (+-1) [R1], below it for the
     command input's reserved rows (58 + one cell, docs/terminal.md) and then 14 .. 14 + one cell (+-1).
  I2 0 frames from 1 s to 3.5 s in that window.
  S1 a fresh window without the key; Settings (Ctrl+,) > General: the row "New windows open with" shows "One" and
     "Two" segments and its text holds "Applies" and a second "window"; a click on "One" makes config.xml hold
     <new_window_panes>1</new_window_panes> exactly once within 3 s (ElementTree text "1" as well), the window
     process is the same and it still has 2 panes (API and pixels: the setting is for the next window).
  I3 0 frames from 1 s to 3.5 s after that change, Settings open.
  S2 the next window, started on that config.xml (byte for byte the file S1 left): 1 pane (API and pixels).
  E1 a config.xml whose line 3 is <new_window_panes>3</new_window_panes> [M]: the window opens with 2 panes (API and
     pixels); Settings shows "line 3" [S] and the word "panes" (the key's name); 3 s later the file is byte for byte
     the same.
  R1 restore [M]: a window without the key and with restore_layout true opens 2 panes; Ctrl+Shift+D (split right)
     makes 3; once its layout.json lists 3 terminals the window and its server are closed. A new window with
     <new_window_panes>1</new_window_panes>, restore_layout true and that layout.json in its config folder opens
     the saved 3 panes (API and pixels), not 1.

On clanky-gpui (CLANKY_BIN, G2): P1, I1, P2, I2 and E1's pane count run; S1, I3, S2 and E1's Settings text move to
G4 (Settings), and R1 (restoring a saved layout) is not part of G2. Its values come from the rulebook
(rulebook-tokens.json), never from the binary: the gap between panes 8 (rulebook:103); the padding inside a pane 12
(rulebook:103; today's 14) on every side of the grid; below the grid's padding, the command input of
1 + 8 + 28 + 8 + 12 = 57 px plus its text line of one cell (hairline, step, h_chip 28, step, its bottom padding 12:
the sum the Q2 gate checks), then 0 .. one cell left over. A pane region here includes the pane's one-pixel edge
(rulebook border_width 1), so every distance from it is one more.
Planted faults (the check that must catch each): nokey (P2: the window's config leaves the key out), noclick (S1:
the "One" segment is not clicked), badvalue (E1: line 3 holds the valid value 1, so 1 pane opens), norestore (R1:
the layout.json is not copied, so the window opens 1 pane), idle (I1: a shell loop prints a line every 0.4 s).
"""
import importlib.util, json, os, re, shutil, subprocess, sys, time
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_C5_WORK', '/srv/devops/build/clanky-c5-gate-run'))
os.environ['CLANKY_C1_WORK'] = str(ROOT)
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
_argv = sys.argv
sys.argv = sys.argv[:1]  # the harness modules read their own --fault from argv; they get none
spec = importlib.util.spec_from_file_location('c1', Path(__file__).with_name('gate-c1-config.py'))
c1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c1)
sys.argv = _argv
q1, t2 = c1.q1, c1.t2
Fail, require, until = c1.Fail, c1.require, c1.until
config_xml = c1.config_xml
APP = c1.APP
W, H = c1.W, c1.H

PANE_BG = '#1b2a3a'
TERM = {'background': PANE_BG, 'foreground': '#d9c8a9', 'cursor': '#f2a541',
        'ansi': ['#2e1f4d', '#c23b3b', '#3bb273', '#d9a21b', '#3b6fd9', '#b03bd9', '#2bb3b3', '#c8b88a',
                 '#5a4a7a', '#ff6b6b', '#6bff9e', '#ffd76b', '#6b9eff', '#e06bff', '#6bfff0', '#fff3c4']}
BASE = dict(startup_animation=False, animations=False, cursor_blink='off', terminal_colors=TERM)
GAP, PAD = 8, 14
INPUT_H = 1 + 8 + 32 + 7 + 10  # command input without its text line (command_bar.rs height)
EDGE = 0  # today's pane region has no edge of its own
GPUI = t2.gate_app.override() is not None
if GPUI:
    # clanky-gpui: the rulebook's pane gap, padding and edge, and its command input (see the docstring).
    _tokens = json.loads((Path(__file__).with_name('window-chrome') / 'rulebook-tokens.json').read_text())
    GAP, PAD = _tokens['pane']['gap']['px'], _tokens['pane']['padding']['px']
    INPUT_H = 1 + 8 + _tokens['heights']['h_chip']['px'] + 8 + PAD
    EDGE = _tokens['pane']['border_width']['device_px']
MAG = (255, 0, 255)
PROG = r'''#!/usr/bin/python3 -I
# c5prog: alternate screen, cursor hidden; magenta cells at (1,1) and at the last cell; quits on "q".
import os, select, termios, tty
fd = 0
old = termios.tcgetattr(fd)
tty.setraw(fd)
w = lambda s: os.write(1, s.encode())
try:
    w("\x1b[?1049h\x1b[?25l\x1b[2J")
    cols, rows = os.get_terminal_size()
    cell = lambda c, r: w(f"\x1b[{r};{c}H\x1b[48;2;255;0;255m \x1b[0m")
    cell(1, 1); cell(cols, rows)
    while True:
        r, _, _ = select.select([fd], [], [], 120)
        if not r or os.read(fd, 64) == b"q":
            break
finally:
    w("\x1b[?25h\x1b[?1049l")
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
'''


# ---------------------------------------------------------------- the window: C1's, keeping its startup panes

class Win(c1.C1Win):
    pass


def _start(self, cfg, extra_env):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('XDG_', 'CLANKY', 'WAYLAND', 'DBUS', 'GNOME', 'GSETTINGS'))}
    self.home = self.w / 'home'
    b = self.home / 'bin'
    b.mkdir(parents=True, exist_ok=True)
    (b / 'c5prog').write_text(PROG)
    (b / 'c5prog').chmod(0o755)
    env.update(HOME=str(self.home), XDG_CONFIG_HOME=str(self.w / 'config'), XDG_DATA_HOME=str(self.w / 'data'),
               XDG_STATE_HOME=str(self.w / 'state'), XDG_RUNTIME_DIR=str(self.w / 'runtime'), SHELL='/bin/sh',
               PATH=f'{b}:{os.environ.get("PATH", "/usr/bin:/bin")}', ENV='',
               TZ='UTC', GSETTINGS_BACKEND='memory', WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1',
               LP_NUM_THREADS='8', VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json',
               CLANKY_FRAME_TRACE=str(self.w / 'frames.csv'))
    self.config = self.w / 'config.xml'
    if isinstance(cfg, dict):
        config_xml.write(self.config, cfg)
    else:
        self.config.write_text(cfg)
    if self.prepare:
        self.prepare(self.w)
    rd, wr = os.pipe()
    x = subprocess.Popen(['Xvfb', ':' + t2.free_display(), '-displayfd', str(wr), '-screen', '0', '1600x1000x24',
                          '-nolisten', 'tcp'], pass_fds=(wr,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    self.procs.append(x)
    os.close(wr)
    env['DISPLAY'] = ':' + os.read(rd, 32).decode().strip()
    os.close(rd)
    self.env = env
    self.sock = self.w / 'window.sock'
    self.err = self.w / 'window.err'
    p = subprocess.Popen([str(APP), '--socket', str(self.sock), '--config', str(self.config)], env=env,
                         cwd=str(self.home), stdout=subprocess.DEVNULL, stderr=self.err.open('wb'))
    self.procs.append(p)
    self.p = p
    try:
        until(lambda: self.sock.exists() or require(p.poll() is None, 'window exited'), 'window socket', 40)
        self.wid = until(lambda: (subprocess.run(['xdotool', 'search', '--pid', str(p.pid)], env=env,
                                                 capture_output=True, text=True).stdout.split() or [None])[0], 'window', 40)
    except Fail as e:
        tail = self.err.read_text(errors='replace').strip().splitlines()[-1:] if self.err.exists() else []
        raise Fail(f'{e}: {" ".join(tail)[:300] or "no error text"}')
    self.x('windowsize', '--sync', self.wid, str(W), str(H))
    self.x('windowmove', '--sync', self.wid, '0', '0')
    self.o = t2.Conn(self.sock)
    self.o.call(3, 'auth', token=Path(str(self.sock) + '.owner').read_text().strip())
    self.server = self.o.call(3, 'hello').get('server_pid')
    until(lambda: len(self.o.call(1, 'list_tabs')['tabs']) >= 1, 'first pane')
    time.sleep(2.5)  # every startup pane is open by now; none is closed here
    self.t0 = time.time()
    self.x('windowfocus', '--sync', self.wid)
    self.x('mousemove', '--window', self.wid, '120', '600')
    time.sleep(.5)


Win._start = _start


def start(name, cfg, label, prepare=None):
    try:
        return Win(name, cfg, prepare=prepare)
    except Fail as e:
        raise Fail(f'{label} ({e})')


# ---------------------------------------------------------------- counting panes

def leaves(layout):
    if 'Leaf' in layout:
        return 1
    s = layout['Split']
    return leaves(s['first']) + leaves(s['second'])


def api_panes(win):
    ws = win.workspace()
    return leaves(ws['groups'][ws['active']]['layout']), len(win.terminals())


def pixel_panes(win, name):
    """Rectangles (x0, y0, x1, y1 inclusive) of the pane regions, top to bottom then left to right."""
    import numpy as np
    from scipy import ndimage
    a = np.asarray(win.shot(name)).astype(int)
    mask = np.abs(a - np.array(t2.rgb(PANE_BG))).max(axis=2) <= 16
    lab, n = ndimage.label(mask)
    parts = []
    for i, sl in enumerate(ndimage.find_objects(lab), 1):
        ys, xs = sl
        h, w = ys.stop - ys.start, xs.stop - xs.start
        if w >= 200 and h >= 20 and (lab[sl] == i).sum() >= .6 * w * h:
            parts.append((xs.start, ys.start, xs.stop - 1, ys.stop - 1))
    out = []
    for r in sorted(parts, key=lambda r: (r[1], r[0])):
        # a pane's own hairline (the command input's, 1-3 rows) splits its region; the parts are one pane
        j = next((k for k, o in enumerate(out) if near(o[0], r[0]) and near(o[2], r[2]) and 0 < r[1] - o[3] <= 4),
                 None)
        if j is None:
            out.append(r)
        else:
            o = out[j]
            out[j] = (min(o[0], r[0]), o[1], max(o[2], r[2]), r[3])
    return sorted([r for r in out if r[3] - r[1] + 1 >= 100], key=lambda r: (r[1], r[0]))


def panes_are(win, n, tag, name):
    """n panes by the owner API and by the pixels; returns the pixel rectangles."""
    try:
        until(lambda: api_panes(win) == (n, n), f'{tag}: {n} pane{"s" if n > 1 else ""} by the owner API', 4)
    except Fail:
        got = api_panes(win)
        raise Fail(f'{tag}: the window opens {n} pane{"s" if n > 1 else ""} by the owner API '
                   f'(observed {got[0]} in the layout, {got[1]} terminals)')
    rects = pixel_panes(win, name)
    require(len(rects) == n, f'{tag}: {n} pane region{"s" if n > 1 else ""} in the pixels (observed {len(rects)}: {rects})')
    return rects


def near(a, b, tol=1):
    return abs(a - b) <= tol


# ---------------------------------------------------------------- config.xml, read independently

def file_panes(path):
    raw = Path(path).read_text()
    try:
        root = ET.fromstring(raw)
    except ET.ParseError:
        return '<malformed>', raw
    els = [e for e in root if e.tag == 'new_window_panes']
    return ((els[0].text or '').strip() if len(els) == 1 else f'<{len(els)} elements>'), raw


def holds_one(path):
    v, raw = file_panes(path)
    return v == '1' and len(re.findall(r'<new_window_panes>\s*1\s*</new_window_panes>', raw)) == 1


# ---------------------------------------------------------------- stories

def area_story():
    win = start('p1', dict(BASE), 'P1 a fresh window')
    try:
        rects = panes_are(win, 2, 'P1 fresh window, no new_window_panes key', 'p1')
        top, bot = rects
        require(near(top[0], bot[0]) and near(top[2], bot[2]),
                f'P1 the two panes are one above the other (x ranges {top[0]}-{top[2]} and {bot[0]}-{bot[2]})')
        gap = bot[1] - top[3] - 1
        require(near(gap, GAP), f'P1 the gap between the panes is {GAP} rows (observed {gap})')
        print(f'ok: P1 2 panes, gap {gap}, top {top}, bottom {bot}')
        if FAULT == 'idle':
            win.click((top[0] + top[2]) / 2, (top[1] + top[3]) / 2)
            win.type("while :; do echo tick; sleep 0.4; done\n")
            time.sleep(1)
        q1.idle(win, 'I1 fresh window')
        print('ok: I1 0 idle frames')
    finally:
        win.close()
    return (top[0], top[1], top[2], bot[3])


def one_pane_story(area):
    cfg = dict(BASE) if FAULT == 'nokey' else dict(BASE, new_window_panes=1)
    win = start('p2', cfg, 'P2 a window with new_window_panes 1')
    try:
        rects = panes_are(win, 1, 'P2 <new_window_panes>1</new_window_panes>', 'p2')
        r = rects[0]
        require(all(near(x, y) for x, y in zip(r, area)),
                f'P2 the one pane fills the pane area {area} (observed {r})')
        win.click((r[0] + r[2]) / 2, (r[1] + r[3]) / 2)
        win.type('c5prog\n')
        time.sleep(1.5)
        im = win.shot('p2prog')
        runs = q1.blobs(im, MAG, (r[0], r[1], r[2] + 1, r[3] + 1), 2)
        require(runs, 'P2 the program painted its magenta cells')
        first_y = min(y for _, _, y in runs)
        last_y = max(y for _, _, y in runs)
        a = [t for t in runs if t[2] - first_y < 60 and t[0] < (r[0] + r[2]) / 2]
        b = [t for t in runs if last_y - t[2] < 60 and t[1] > (r[0] + r[2]) / 2]
        require(a and b, f'P2 two magenta cells, top left and bottom right (runs {runs[:4]})')
        ax0, ax1 = min(t[0] for t in a), max(t[1] for t in a)
        ay0, ay1 = min(t[2] for t in a), max(t[2] for t in a)
        bx1, by1 = max(t[1] for t in b), max(t[2] for t in b)
        cw, ch = ax1 - ax0 + 1, ay1 - ay0 + 1
        inset = EDGE + PAD
        require(near(ax0 - r[0], inset) and near(ay0 - r[1], inset),
                f'P2 cell (1,1) is {inset} px inside the pane (observed {ax0 - r[0]} left, {ay0 - r[1]} top)')
        # Below the grid sit the command input's reserved rows (docs/terminal.md: "Its rows stay reserved
        # meanwhile, so the terminal is not resized at each command"); one line of input is
        # command_bar::height = 58 px + one cell at scale 1.
        bar = INPUT_H + ch
        right, below = r[2] - bx1, r[3] - by1
        require(inset - 1 <= right <= inset + cw + 1 and inset + bar - 1 <= below <= inset + bar + ch + 1,
                f'P2 right of the last cell the pane goes on for {inset} .. {inset} + one cell, below it for the '
                f'input ({bar}) + {inset} .. {inset} + one cell (cell {cw} x {ch}; observed {right} right, {below} below)')
        print(f'ok: P2 1 pane {r} fills the area; padding {ax0 - r[0]}/{ay0 - r[1]}, {right}/{below}')
        win.type('q')
        time.sleep(1)
        q1.idle(win, 'I2 one-pane window')
        print('ok: I2 0 idle frames')
    finally:
        win.close()


def settings_story():
    win = start('s1', dict(BASE), 'S1 a fresh window')
    try:
        panes_are(win, 2, 'S1 fresh window', 's1a')
        c1.open_settings(win, 'S1')
        c1.page(win, 'General')
        lab, im = c1.label_at(win, 'New windows open with')
        band = c1.row_band(win, lab)
        hit = None
        for words in c1.band_words(win, im, band):
            one = [w for w in words if w[1] > lab[2] + 8 and c1.token_ok(w[0], 'one')]
            two = [w for w in words if w[1] > lab[2] + 8 and c1.token_ok(w[0], 'two')]
            if one and two:
                hit = one[-1]
                break
        require(hit, f'S1 the row "New windows open with" has "One" and "Two" segments '
                     f'(read {c1.all_text(im, band)[0][-100:]!r})')
        texts = c1.all_text(im, band)
        require(any('appl' in t and len(re.findall(r'window', t)) >= 2 for t in texts),
                f'S1 the row says it applies to the next new window (read {texts[0][-120:]!r})')
        if FAULT != 'noclick':
            win.click(*q1.centre(hit))
        try:
            until(lambda: holds_one(win.config), 'S1 config.xml holds the value', 3)
        except Fail:
            v, raw = file_panes(win.config)
            raise Fail(f'S1 a click on "One" writes <new_window_panes>1</new_window_panes> to config.xml '
                       f'(file: {v})')
        require(win.alive() and win.p.pid == win.pid0, 'S1 same window process')
        c1.close_settings(win)
        panes_are(win, 2, 'S1 the open window keeps its 2 panes', 's1b')
        print('ok: S1 Settings > General "One" writes new_window_panes 1; the open window keeps 2 panes')
        c1.open_settings(win, 'I3')
        c1.page(win, 'General')
        q1.idle(win, 'I3 Settings open after the change')
        print('ok: I3 0 idle frames')
        written = win.config.read_text()
    finally:
        win.close()
    win = start('s2', written, 'S2 the next window')
    try:
        panes_are(win, 1, 'S2 the next window follows the setting', 's2')
        print('ok: S2 the next window opens 1 pane')
    finally:
        win.close()


def bad_story():
    value = '1' if FAULT == 'badvalue' else '3'
    text = (c1.GOOD_HEAD + f'  <new_window_panes>{value}</new_window_panes>\n'
            '  <startup_animation>false</startup_animation>\n  <animations>false</animations>\n'
            '  <cursor_blink>off</cursor_blink>\n' + config_xml.dump(dict(terminal_colors=TERM)).splitlines()[2]
            + '\n</clanky>\n')
    require(text.splitlines()[2].strip() == f'<new_window_panes>{value}</new_window_panes>', 'E1 input: line 3')
    win = start('e1', text, 'E1 a bad value does not stop the window')
    try:
        panes_are(win, 2, 'E1 <new_window_panes>3</new_window_panes> falls back to 2 panes', 'e1')
        if GPUI:
            print('ok: E1 a bad value opens 2 panes (its Settings text is checked in G4)')
            return
        c1.open_settings(win, 'E1')
        texts = c1.all_text(win.shot('e1s'), win.frame)
        require(any(re.search(r'line\s*:?\s*3\b', t) and 'panes' in t for t in texts),
                f'E1 Settings shows the error at "line 3" naming the key (read {texts[0][:160]!r})')
        time.sleep(3)
        require(win.config.read_text() == text, 'E1 the file is not rewritten')
        print('ok: E1 a bad value opens 2 panes and Settings shows line 3')
    finally:
        win.close()


def restore_story():
    win = start('r1', dict(BASE), 'R1 a window that saves its layout')
    saved = win.w / 'config/clanky/layout.json'

    def three():
        try:
            return len(json.loads(saved.read_text())['terminals']) == 3
        except (OSError, ValueError, KeyError):
            return False
    try:
        panes_are(win, 2, 'R1 first window', 'r1a')
        win.key('ctrl+shift+d')
        panes_are(win, 3, 'R1 Ctrl+Shift+D splits to 3 panes', 'r1b')
        until(three, 'R1 layout.json lists 3 terminals', 6)
    finally:
        win.close()

    def prepare(w):
        if FAULT != 'norestore':
            d = w / 'config/clanky'
            d.mkdir(parents=True, exist_ok=True)
            d.chmod(0o700)
            shutil.copyfile(saved, d / 'layout.json')
            (d / 'layout.json').chmod(0o600)
    win = start('r2', dict(BASE, new_window_panes=1, restore_layout=True), 'R1 a window restoring that layout',
                prepare)
    try:
        panes_are(win, 3, 'R1 the restored layout wins over <new_window_panes>1</new_window_panes>', 'r2')
        print('ok: R1 the restored 3-pane layout wins')
    finally:
        win.close()


def main():
    for tool in ('Xvfb', 'xdotool', 'ffmpeg', 'tesseract'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True, mode=0o700)
    if FAULT is None:
        area = area_story()
        one_pane_story(area)
        if not GPUI:
            settings_story()
        bad_story()
        if not GPUI:
            restore_story()
    elif FAULT == 'nokey':
        one_pane_story(area_story())
    elif FAULT == 'idle':
        area_story()
    else:
        stories = {'noclick': settings_story, 'badvalue': bad_story, 'norestore': restore_story}
        require(FAULT in stories, f'known fault {FAULT}')
        stories[FAULT]()
    shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: C5 new windows open with one or two panes')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: C5 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: C5 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
