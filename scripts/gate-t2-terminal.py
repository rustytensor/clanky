#!/usr/bin/env python3
"""T2 gate: the terminal feels like Ptyxis (Marc, 2026-11-12: "the same look and feel as Ptyxis").

Usage: gate-t2-terminal.py [--fault scale|blink|colors|keys]
A fault changes the run's input on purpose; the gate must then FAIL.

Everything runs in a real Clanky window on a private Xvfb display at scale 1, with software Vulkan.

Interface fixed by this gate (config keys the work must accept):
  font_family "IBM Plex Mono" (any installed system monospace font), font_size_pt 11,
  cell_height_scale 1.25 (default 1.0), cell_width_scale (default 1.0),
  cursor_blink "system" (default) | "on" | "off", cursor_blink_time_ms (default 1200),
  cursor_blink_timeout_s (default 10). "system" follows GNOME's org.gnome.desktop.interface cursor-blink,
  cursor-blink-time and cursor-blink-timeout when readable, else on / 1200 / 10.
  terminal_colors {"background","foreground","cursor","ansi":[16]}: overrides the terminal's colours only;
  the rest of the window keeps the theme.
  `clanky terminal-colors import FILE.palette` prints the [Dark] section of a Ptyxis palette as that JSON object
  (keys background, foreground, cursor, ansi = Color0..Color15, lowercase #rrggbb). Checked as I1.

Expected results, fixed before the work from independent sources:
  K1 TERM in a pane is xterm-256color.
  K2 bytes a program receives per key, from `infocmp -1 xterm-256color` and xterm's ctlseqs (normal cursor mode
     for arrows/Home/End; application mode after DECCKM ESC[?1h): see KEYS_NORMAL, KEYS_APP, KEYS_BOTH below.
  K3 Midnight Commander (/usr/bin/mc -u): F9 then Down opens the menu bar's Left menu ("Listing format" on screen;
     the same keys in tmux 3.x show it, checked 2026-11-12); after
     Escape Escape and F10 it exits and the shell prompt shows no stray "[A"/"[B"/"[C"/"[D".
  C1 terminal_colors from Marc's Ptyxis palette "riley": cell backgrounds for SGR 41-47 are exactly the palette's
     Color1-Color7 (each channel within 2), the empty pane background is #151a23, and the sidebar keeps the
     theme's own sidebar colour (terminal colours are independent).
  C2 theme stone-light, no terminal_colors: SGR 41-46 cells are exactly stone-light's ansi[1..6] (within 2).
     Clanky 1.2.2 drew colours brighter (theme red #f0716a shown as #f8b2ac), so this fails before the fix.
  F1 IBM Plex Mono 11 pt with cell_height_scale 1.25: 14 cells are 126 px wide (within 2) and one cell row is
     26 px tall (within 1). Measured from Ptyxis 50.1 / VTE 0.84.1 with the same font, size and scale:
     9 x 26 px cells (/srv/devops/build/ref/ptyxis-run/ptyxis.png).
  F2 bold (SGR 1) text has at least 1.3 times the ink pixels of the same text in regular (Ptyxis measured 1.57).
  B1 default config (cursor_blink "system", no GNOME settings readable in the test): after one key press the
     window draws at least 14 frames in the next 10 s (a 600 ms half-cycle gives 16-17) and 0 frames from
     11 s to 16 s, and the cursor cell shows the cursor colour at the end (blinking stops with it visible).
  B2 cursor_blink "off": 0 frames from 1 s to 6 s after a key press.
"""
import os, re, shutil, socket, json, subprocess, sys, time
from pathlib import Path

TARGET = Path(os.environ['CARGO_TARGET_DIR'])
APP = TARGET / 'debug/clanky'
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
ROOT = Path(os.environ.get('CLANKY_T2_WORK', '/srv/devops/build/clanky-t2-gate'))
RILEY = {'background': '#151a23', 'foreground': '#d6d2c4', 'cursor': '#f0ece0',
         'ansi': ['#151a23', '#e5776a', '#9fc184', '#e3c169', '#7fa7d8', '#bf98d9', '#68bfb5', '#d6d2c4',
                  '#5d687c', '#f29488', '#b8d6a0', '#f0d48c', '#a0c0e8', '#d4b3ea', '#8ad4cb', '#f0ece0']}
RILEY_PALETTE_FILE = '[Palette]\nName=Riley (mission-control)\nPrimary=false\n\n[Dark]\nBackground=#151a23\nForeground=#d6d2c4\nCursor=#f0ece0\nTitlebarBackground=#10141b\nTitlebarForeground=#d6d2c4\nColor0=#151a23\nColor1=#e5776a\nColor2=#9fc184\nColor3=#e3c169\nColor4=#7fa7d8\nColor5=#bf98d9\nColor6=#68bfb5\nColor7=#d6d2c4\nColor8=#5d687c\nColor9=#f29488\nColor10=#b8d6a0\nColor11=#f0d48c\nColor12=#a0c0e8\nColor13=#d4b3ea\nColor14=#8ad4cb\nColor15=#f0ece0\n\n[Light]\nBackground=#151a23\nForeground=#d6d2c4\nCursor=#f0ece0\nTitlebarBackground=#10141b\nTitlebarForeground=#d6d2c4\nColor0=#151a23\nColor1=#e5776a\nColor2=#9fc184\nColor3=#e3c169\nColor4=#7fa7d8\nColor5=#bf98d9\nColor6=#68bfb5\nColor7=#d6d2c4\nColor8=#5d687c\nColor9=#f29488\nColor10=#b8d6a0\nColor11=#f0d48c\nColor12=#a0c0e8\nColor13=#d4b3ea\nColor14=#8ad4cb\nColor15=#f0ece0\n'  # Marc's Ptyxis palette, copied 2026-11-12
STONE_LIGHT_ANSI = json.loads(Path(__file__).resolve().parents[1].joinpath('assets/themes/stone-light.json').read_text())['ansi']
E = '\x1b'
KEYS_NORMAL = [('Up', E + '[A'), ('Down', E + '[B'), ('Right', E + '[C'), ('Left', E + '[D'),
               ('Home', E + '[H'), ('End', E + '[F')]
KEYS_APP = [('Up', E + 'OA'), ('Down', E + 'OB'), ('Right', E + 'OC'), ('Left', E + 'OD'),
            ('Home', E + 'OH'), ('End', E + 'OF')]
KEYS_BOTH = [('F1', E + 'OP'), ('F2', E + 'OQ'), ('F3', E + 'OR'), ('F4', E + 'OS'), ('F5', E + '[15~'),
             ('F6', E + '[17~'), ('F7', E + '[18~'), ('F8', E + '[19~'), ('F9', E + '[20~'), ('F10', E + '[21~'),
             ('F12', E + '[24~'), ('Insert', E + '[2~'), ('Delete', E + '[3~'), ('Prior', E + '[5~'),
             ('Next', E + '[6~'), ('shift+Up', E + '[1;2A'), ('ctrl+Right', E + '[1;5C'), ('alt+x', E + 'x'),
             ('BackSpace', '\x7f'), ('Tab', '\t'), ('shift+Tab', E + '[Z'), ('Escape', E)]
# Reads keys raw and logs each read() as JSON, one line per read. Optional argv[2] "app" sets DECCKM first.
LOGGER = r'''
import json, os, sys, termios, tty, select
out = open(sys.argv[1], "a")
fd = sys.stdin.fileno(); old = termios.tcgetattr(fd); tty.setraw(fd)
if len(sys.argv) > 2 and sys.argv[2] == "app": os.write(1, b"\x1b[?1h")
os.write(1, b"LOGGER-READY\r\n")
try:
    while True:
        r, _, _ = select.select([fd], [], [], 30)
        if not r: break
        data = os.read(fd, 64)
        if data == b"q": break
        out.write(json.dumps(data.decode("latin-1")) + "\n"); out.flush()
finally:
    os.write(1, b"\x1b[?1l"); termios.tcsetattr(fd, termios.TCSADRAIN, old)
'''


class Fail(Exception):
    pass


def require(ok, label):
    if not ok:
        raise Fail(label)


def until(fn, label, timeout=15):
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


class Window:
    """One private Xvfb display with one Clanky window; close() stops both and the window's server."""

    def __init__(self, name, **cfg):
        self.w = ROOT / name
        self.w.mkdir(mode=0o700)
        for d in ('home', 'config', 'data', 'state', 'runtime'):
            (self.w / d).mkdir(mode=0o700)
        self.procs, self.server = [], None
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(('XDG_', 'CLANKY', 'WAYLAND', 'DBUS', 'GNOME', 'GSETTINGS'))}
        env.update(HOME=str(self.w / 'home'), XDG_CONFIG_HOME=str(self.w / 'config'), XDG_DATA_HOME=str(self.w / 'data'),
                   XDG_STATE_HOME=str(self.w / 'state'), XDG_RUNTIME_DIR=str(self.w / 'runtime'), SHELL='/bin/sh',
                   GSETTINGS_BACKEND='memory', WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1', LP_NUM_THREADS='8',
                   VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json', CLANKY_FRAME_TRACE=str(self.w / 'frames.csv'))
        config = self.w / 'config.json'
        config.write_text(json.dumps(dict(restore_layout=False, startup_animation=False, history=False, rails=True, **cfg)))
        rd, wr = os.pipe()
        x = subprocess.Popen(['Xvfb', '-displayfd', str(wr), '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'],
                             pass_fds=(wr,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.procs.append(x)
        os.close(wr)
        env['DISPLAY'] = ':' + os.read(rd, 32).decode().strip()
        os.close(rd)
        self.env = env
        self.sock = self.w / 'window.sock'
        p = subprocess.Popen([str(APP), '--socket', str(self.sock), '--config', str(config)], env=env, cwd=str(self.w),
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.procs.append(p)
        until(lambda: self.sock.exists() or require(p.poll() is None, 'window startup'), 'window socket', 40)
        self.wid = until(lambda: (subprocess.run(['xdotool', 'search', '--pid', str(p.pid)], env=env,
                                                 capture_output=True, text=True).stdout.split() or [None])[0], 'window', 40)
        self.x('windowsize', '--sync', self.wid, '1400', '900')
        self.x('windowmove', '--sync', self.wid, '0', '0')
        self.o = Conn(self.sock)
        self.o.call(3, 'auth', token=Path(str(self.sock) + '.owner').read_text().strip())
        self.server = self.o.call(3, 'hello').get('server_pid')
        self.tab = until(lambda: next(iter(t['tab_id'] for t in self.o.call(1, 'list_tabs')['tabs']), None), 'first pane')
        time.sleep(2)
        self.x('windowfocus', '--sync', self.wid)
        self.x('mousemove', '--window', self.wid, '700', '450', 'click', '1')
        time.sleep(.5)

    def x(self, *args):
        subprocess.run(['xdotool', *args], env=self.env, check=True, stdout=subprocess.DEVNULL)

    def type(self, text):
        self.x('type', '--delay', '25', text)

    def key(self, *keys):
        for k in keys:
            self.x('key', k)
            time.sleep(.3)

    def screen(self):
        s = self.o.call(4, 'screen', tab_id=self.tab, history_offset=0, history_limit=64)
        lines = s.get('history', {}).get('lines', []) + s.get('lines', [])
        return '\n'.join(l if isinstance(l, str) else l.get('text', '') for l in lines)

    def shot(self, name):
        path = self.w / f'{name}.png'
        subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-f', 'x11grab', '-video_size', '1400x900', '-i',
                        self.env['DISPLAY'] + '+0,0', '-frames:v', '1', str(path)], env=self.env, check=True,
                       stdin=subprocess.DEVNULL)
        from PIL import Image
        return Image.open(path).convert('RGB')

    def frames(self):
        f = self.w / 'frames.csv'
        lines = f.read_text().splitlines() if f.exists() else []
        clock = next((int(l.split(',')[1]) for l in lines if l.startswith('clock,')), 0)
        return [(clock + int(l.split(',')[1])) / 1e6 for l in lines if l.startswith(('phases,', 'frame,'))]

    def close(self):
        if self.server:
            try:
                os.kill(self.server, 15)
            except ProcessLookupError:
                pass
        for p in reversed(self.procs):
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    p.kill()


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def near(a, b, tol=2):
    return all(abs(x - y) <= tol for x, y in zip(a, b))


def find_run(im, colour, y_range=None):
    """First pixel of `colour` (within 2) scanning rows top to bottom; returns (x, y) or None."""
    W, H = im.size
    want = rgb(colour)
    for y in (y_range or range(H)):
        for x in range(W):
            if near(im.getpixel((x, y)), want):
                return x, y
    return None


def keys_story(win):
    win.type('echo TERM=$TERM\n')
    until(lambda: 'TERM=xterm-256color' in win.screen(), 'K1 TERM is xterm-256color')
    logger = win.w / 'logger.py'
    logger.write_text(LOGGER)
    for mode, table in (('normal', KEYS_NORMAL + KEYS_BOTH), ('app', KEYS_APP + KEYS_BOTH)):
        log = win.w / f'keys-{mode}.log'
        arg = '' if (mode == 'normal' or FAULT == 'keys') else ' app'
        win.type(f'clear; python3 {logger} {log}{arg}\n')
        until(lambda: 'LOGGER-READY' in win.screen(), f'K2 key logger started ({mode})')
        for name, _ in table:
            win.key(name)
        win.type('q')
        time.sleep(1)
        got = [json.loads(l) for l in log.read_text().splitlines()] if log.exists() else []
        want = [seq for _, seq in table]
        bad = [f'{name}: got {got[i]!r}' if i < len(got) else f'{name}: nothing'
               for i, (name, seq) in enumerate(table) if i >= len(got) or got[i] != seq]
        require(not bad and len(got) == len(want), f'K2 key bytes ({mode}): ' + '; '.join(bad[:6]))
    win.type('clear; /usr/bin/mc -u\n')
    until(lambda: 'Name' in win.screen() and 'Size' in win.screen(), 'K3 mc started', 20)
    time.sleep(1)
    win.key('F9', 'Down')
    until(lambda: 'Listing format' in win.screen(), 'K3 F9 then Down opens the mc Left menu', 8)
    win.key('Escape', 'Escape')
    time.sleep(1)
    win.key('F10')
    until(lambda: 'Name' not in win.screen(), 'K3 F10 quits mc', 8)
    win.type('echo MC-DONE\n')
    until(lambda: 'MC-DONE' in win.screen(), 'K3 shell back')
    require(not re.search(r'\[[ABCD]\b', win.screen().split('MC-DONE')[0][-200:]), 'K3 no stray arrow text')


def palette_line():
    blocks = ''.join(f'\\033[{40 + i}m  ' for i in range(1, 8)) + '\\033[0m'
    return f"clear; printf 'MMMMMMMM\\n\\033[1mMMMMMMMM\\033[0m\\n{blocks}\\n'; sleep 30\n"


def colours_and_font(win, ansi, label, check_font, check_independent=None):
    win.type(palette_line())
    time.sleep(2)
    im = win.shot(label)
    start = find_run(im, ansi[1], range(60, 800))
    require(start, f'C {label}: red cell colour {ansi[1]} found on screen')
    x0, y0 = start
    W, H = im.size
    row = [im.getpixel((x, y0 + 3)) for x in range(x0, min(W, x0 + 400))]
    for i in range(1, 7):
        require(any(near(p, rgb(ansi[i])) for p in row), f'C {label}: SGR {40 + i} cell is {ansi[i]}')
    if check_independent:
        bg, sidebar = check_independent
        require(near(im.getpixel((x0 + 700, y0 + 200)), rgb(bg)), f'C {label}: empty pane background is {bg}')
        require(not near(im.getpixel((20, 600)), rgb(bg)), f'C {label}: sidebar keeps the theme colour')
    if not check_font:
        return
    x_end = x0
    while x_end + 1 < W and any(near(im.getpixel((x_end + 1, y0 + 3)), rgb(ansi[i])) for i in range(1, 8)):
        x_end += 1
    width = x_end - x0 + 1
    require(abs(width - 126) <= 2, f'F1 14 cells are 126 px wide (observed {width})')
    y_end = y0
    while y_end + 1 < H and near(im.getpixel((x0 + 2, y_end + 1)), rgb(ansi[1])):
        y_end += 1
    height = y_end - y0 + 1
    require(abs(height - 26) <= 1, f'F1 one cell row is 26 px tall (observed {height})')
    bgc = im.getpixel((x0 + 700, y0 + 200))

    def ink(y):
        return sum(1 for yy in range(y, y + height) for xx in range(x0, x0 + 72)
                   if sum(im.getpixel((xx, yy))) > sum(bgc) + 150)
    regular, bold = ink(y0 - 2 * height), ink(y0 - height)
    require(regular > 0 and bold >= 1.3 * regular, f'F2 bold has 1.3x the ink of regular ({bold} vs {regular})')


def blink(win, mode):
    win.type('clear\n')
    time.sleep(13)  # let any blink from startup typing time out
    t0 = time.time()
    win.key('space')
    time.sleep(16.5)
    f = win.frames()
    early = [t for t in f if t0 + .3 <= t <= t0 + 10]
    late = [t for t in f if t0 + 11 <= t <= t0 + 16]
    if mode == 'off':
        require(not [t for t in f if t0 + 1 <= t <= t0 + 6], 'B2 blink off: no frames 1-6 s after a key')
        return
    require(len(early) >= 14, f'B1 cursor blinks for 10 s after a key ({len(early)} frames)')
    require(not late, f'B1 blinking stops after 10 s ({len(late)} frames from 11 to 16 s)')


def main():
    shutil.rmtree(ROOT, ignore_errors=True)
    ROOT.mkdir(parents=True, mode=0o700)
    riley = json.loads(json.dumps(RILEY))
    if FAULT == 'colors':
        riley['ansi'][1] = '#000000'
    scale = 1.0 if FAULT == 'scale' else 1.25
    stories = [
        ('keys', dict(theme='dark', theme_dark='ink'), keys_story),
        ('riley', dict(theme='dark', theme_dark='stone-dark', font_family='IBM Plex Mono', font_size_pt=11,
                       cell_height_scale=scale, terminal_colors=riley, cursor_blink='off'),
         lambda w: colours_and_font(w, RILEY['ansi'], 'riley', True, (RILEY['background'], None))),
        ('light', dict(theme='light', theme_light='stone-light', cursor_blink='off'),
         lambda w: colours_and_font(w, STONE_LIGHT_ANSI, 'light', False)),
        ('blink-default', dict(theme='dark', **({'cursor_blink': 'off'} if FAULT == 'blink' else {})),
         lambda w: blink(w, 'default')),
        ('blink-off', dict(theme='dark', cursor_blink='off'), lambda w: blink(w, 'off')),
    ]
    if FAULT == 'keys':
        stories = stories[:1]
    try:
        if FAULT is None:
            pal = ROOT / 'riley.palette'
            pal.write_text(RILEY_PALETTE_FILE)
            r = subprocess.run([str(APP), 'terminal-colors', 'import', str(pal)], capture_output=True, text=True, timeout=30)
            require(r.returncode == 0, 'I1 terminal-colors import exit code')
            try:
                got = json.loads(r.stdout)
            except ValueError:
                got = None
            require(got == RILEY, 'I1 imported Ptyxis palette equals the [Dark] section')
            print('ok: import')
        for name, cfg, story in stories:
            win = Window(name, **cfg)
            try:
                story(win)
            finally:
                win.close()
            print(f'ok: {name}')
    except Fail as e:
        print(f'FAIL: T2 {e}')
        return 1
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: T2 unexpected {type(e).__name__}: {e}')
        return 1
    if FAULT is None:
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: T2 terminal')
    return 0


if __name__ == '__main__':
    sys.exit(main())
