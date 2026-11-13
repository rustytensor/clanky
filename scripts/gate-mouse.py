#!/usr/bin/env python3
"""Mouse gate: the mouse works in a terminal pane as it does in Ptyxis (Marc, 2026-11-13: "The mouse needs to work
the same way as it does in Ptyxis and other high-quality terminals, including with Midnight Commander").

Usage: gate-mouse.py [--fault encoding|shift|wheel|select|mc]
A fault changes the run's input on purpose; the gate must then FAIL.

Runs in a real Clanky window on a private Xvfb display (the T2 gate's harness), IBM Plex Mono 11 pt at 1.25,
cursor blink off. Cell positions are measured, not assumed: a program paints a 24-bit magenta cell at two known
cells and the gate finds them in a screenshot.

Expected results, fixed before the work from independent sources:
  Reports follow xterm's ctlseqs "Mouse Tracking" and VTE 0.84 src/vte.cc feed_mouse_event: cb = 0 left, 1 middle,
  2 right, 64/65 wheel up/down, +4 Shift, +8 Alt, +16 Ctrl, +32 motion; 1-based column;row of the pane's screen.
  R1 mode 1000+1006 (SGR): left click at cell 10;5 sends exactly ESC[<0;10;5M then ESC[<0;10;5m; middle sends
     ESC[<1;10;5M / m; right ESC[<2;10;5M / m; Ctrl+left ESC[<16;10;5M / m; Alt+left ESC[<8;10;5M / m.
  R2 mode 1000 alone (X10 bytes): left click at 10;5 sends ESC[M + bytes 32,42,37 then ESC[M + bytes 35,42,37
     (release is button 3).
  R3 mode 1002+1006 (what mc turns on: it sends ESC[?1002h and ESC[?1006h, seen in a pty 2026-11-13): press at
     10;5, move with the button held to 13;7, release: ESC[<0;10;5M, then one or more ESC[<32;c;rM ending at
     ESC[<32;13;7M, then ESC[<0;13;7m. Moving with no button held sends nothing.
  R4 mode 1003+1006: moving with no button from 10;5 to 13;7 sends ESC[<35;c;rM reports ending at ESC[<35;13;7M.
  R5 wheel in mode 1002+1006: one notch up at 10;5 sends exactly ESC[<64;10;5M, one notch down ESC[<65;10;5M
     (VTE: one report per notch).
  R6 Shift overrides reporting (VTE: "If we're in event mode, and the user held down the shift key, we start
     selecting"): in mode 1002+1006, Shift+double-click on "beta" sends the program nothing and selects "beta".
  W1 alternate screen (1049), application cursor keys (1h), no mouse mode: one wheel notch down sends exactly
     n times ESC OB, n = max(1, ceil(rows / 10)) (VTE widget_mouse_scroll, alternate scroll mode 1007 on by default);
     one notch up sends n times ESC OA.
  W2 normal screen, no mouse mode: wheel sends the program nothing.
  W3 Ctrl+wheel up zooms in (Marc's Ptyxis has enable-zoom-scroll-ctrl true): the measured cell gets wider.
  S1 no mouse mode, line "alpha beta gamma /srv/devops/x-y.txt" at row 3: dragging from the left part of column 1
     to the right part of column 10 makes the PRIMARY selection "alpha beta".
  S2 double-click on "beta" selects "beta"; double-click inside the path selects "/srv/devops/x-y.txt" (VTE's
     default word characters "-#%&+,./=?@\\_~", Ptyxis word-char-exceptions is unset); triple-click selects the
     whole line (one trailing newline allowed). The program receives nothing for any of these.
  S3 the selected cells are drawn differently from unselected ones; a single click elsewhere clears it.
  S4 Ctrl+Shift+C copies the selection to CLIPBOARD; Ctrl+Shift+V sends the CLIPBOARD to the program, wrapped in
     ESC[200~ ... ESC[201~ when the program turned on bracketed paste (2004).
  S5 middle click with no mouse mode sends the PRIMARY selection to the program.
  M1 Midnight Commander (/usr/bin/mc -u): a click on "Left" in its menu bar shows "Listing format"; after
     Escape Escape, a click on the "Quit" label in its bottom bar exits mc. Both checked against mc itself in a
     plain 24x80 pty with xterm's byte format, SGR and X10, 2026-11-13.
  I1 0 frames from 1 s to 4 s after the pointer stops moving over a pane (cursor blink off).
"""
import importlib.util, json, math, os, re, shutil, subprocess, sys, time
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_MOUSE_WORK', '/srv/devops/build/clanky-mouse-gate-run'))
os.environ['CLANKY_T2_WORK'] = str(ROOT)
spec = importlib.util.spec_from_file_location('t2', Path(__file__).with_name('gate-t2-terminal.py'))
t2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t2)
Fail, require, until, Window = t2.Fail, t2.require, t2.until, t2.Window
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
E = '\x1b'
LINE = 'alpha beta gamma /srv/devops/x-y.txt'
MAGENTA = (255, 0, 255)

# A program that turns on the modes in argv[2] (e.g. "1002;1006", "-" for none, "1049;1" for alternate screen with
# application cursor keys), paints LINE at row 3 and magenta cells, logs every read() as JSON, and stops on "q".
PROG = r'''
import json, os, sys, termios, tty, select
out = open(sys.argv[1], "a"); modes = sys.argv[2]; mark = sys.argv[3]
fd = sys.stdin.fileno(); old = termios.tcgetattr(fd); tty.setraw(fd)
w = lambda s: os.write(1, s.encode())
if modes != "-": w("\x1b[?" + modes + "h")
w("\x1b[2J\x1b[H")
rows, cols = os.get_terminal_size()
w("\x1b[3;1H" + "alpha beta gamma /srv/devops/x-y.txt")
for cell in mark.split(","):
    if cell:
        c, r = cell.split(":"); w(f"\x1b[{r};{c}H\x1b[48;2;255;0;255m \x1b[0m")
open(sys.argv[1] + ".size", "w").write(f"{rows} {cols}")
w("\x1b[1;1HREADY")
try:
    while True:
        r, _, _ = select.select([fd], [], [], 60)
        if not r: break
        data = os.read(fd, 4096)
        if data == b"q": break
        out.write(json.dumps(data.decode("latin-1")) + "\n"); out.flush()
finally:
    if modes != "-": w("\x1b[?" + modes + "l")
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
'''


class Pane:
    """Runs PROG in the window's pane; measures cell geometry from two magenta cells."""

    def __init__(self, win):
        self.win, self.n, self.geo = win, 0, None
        (win.w / 'prog.py').write_text(PROG)

    def start(self, modes, mark=''):
        self.n += 1
        self.log = self.win.w / f'prog-{self.n}.log'
        self.win.type(f'clear; python3 {self.win.w / "prog.py"} {self.log} "{modes}" "{mark}"\n')
        size = Path(str(self.log) + '.size')
        until(lambda: size.exists() and size.read_text().strip(), f'program started ({modes})')
        self.rows, self.cols = map(int, size.read_text().split())
        time.sleep(.5)
        self.seen = 0

    def stop(self):
        self.win.type('q')
        time.sleep(.5)

    def bytes(self):
        """Everything the program read since the last call."""
        time.sleep(.6)
        got = [json.loads(l) for l in self.log.read_text().splitlines()] if self.log.exists() else []
        new, self.seen = ''.join(got[self.seen:]), len(got)
        return new

    def measure(self):
        """Cell (10,5) and (30,12) are painted magenta; returns origin and cell size in window pixels."""
        self.start('-', '10:5,30:12')
        im = self.win.shot(f'measure-{self.n}')
        W, H = im.size
        px = im.load()
        hits = [(x, y) for y in range(0, H) for x in range(0, W) if px[x, y] == MAGENTA]
        require(hits, 'measure: magenta cells found on screen')
        a = [(x, y) for x, y in hits if y < min(y for _, y in hits) + 200 and x < min(x for x, _ in hits) + 100]
        b = [(x, y) for x, y in hits if (x, y) not in set(a)]
        require(a and b, 'measure: two magenta cells')
        ax, ay = min(x for x, _ in a), min(y for _, y in a)
        bx, by = min(x for x, _ in b), min(y for _, y in b)
        cw, ch = (bx - ax) / 20, (by - ay) / 7
        require(cw > 3 and ch > 6, f'measure: cell size {cw}x{ch}')
        self.geo = (ax - 9 * cw, ay - 4 * ch, cw, ch)  # pixel of cell (1,1)'s top-left
        self.stop()
        return cw, ch

    def at(self, col, row, fx=.5, fy=.5):
        x0, y0, cw, ch = self.geo
        return int(x0 + (col - 1 + fx) * cw), int(y0 + (row - 1 + fy) * ch)

    def move(self, col, row, fx=.5, fy=.5):
        x, y = self.at(col, row, fx, fy)
        self.win.x('mousemove', '--window', self.win.wid, str(x), str(y))
        time.sleep(.15)

    def click(self, col, row, button=1, mods=(), repeat=1, fx=.5):
        self.move(col, row, fx)
        for m in mods:
            self.win.x('keydown', m)
        self.win.x('click', '--repeat', str(repeat), '--delay', '80', str(button))
        for m in reversed(mods):
            self.win.x('keyup', m)
        time.sleep(.3)


def reports(data):
    return re.findall(r'\x1b\[<(\d+);(\d+);(\d+)([Mm])', data)


def sgr(cb, c, r, kind='M'):
    return f'{E}[<{cb};{c};{r}{kind}'


def primary(win, sel='primary'):
    out = subprocess.run(['xclip', '-o', '-selection', sel], env=win.env, capture_output=True, text=True, timeout=5)
    return out.stdout if out.returncode == 0 else None


def set_clip(win, text, sel):
    subprocess.run(['xclip', '-i', '-selection', sel], env=win.env, input=text, text=True, timeout=5, check=True)


def report_story(win, p):
    modes = '1000' if FAULT == 'encoding' else '1000;1006'
    p.start(modes)
    for button, mods, cb in ((1, (), 0), (2, (), 1), (3, (), 2), (1, ('ctrl',), 16), (1, ('alt',), 8)):
        p.click(10, 5, button, mods)
        got = p.bytes()
        want = sgr(cb, 10, 5) + sgr(cb, 10, 5, 'm')
        require(got == want, f'R1 SGR report button {button} {"+".join(mods)}: got {got!r}, want {want!r}')
    p.stop()
    p.start('1000')
    p.click(10, 5)
    got, want = p.bytes(), f'{E}[M' + chr(32) + chr(42) + chr(37) + f'{E}[M' + chr(35) + chr(42) + chr(37)
    require(got == want, f'R2 X10 report: got {got!r}, want {want!r}')
    p.stop()
    p.start('1002;1006')
    p.move(10, 5)
    win.x('mousedown', '1'); time.sleep(.2)
    p.move(13, 7)
    win.x('mouseup', '1')
    got = reports(p.bytes())
    ok = (len(got) >= 3 and got[0] == ('0', '10', '5', 'M') and got[-1] == ('0', '13', '7', 'm')
          and all(g[0] == '32' and g[3] == 'M' for g in got[1:-1]) and got[-2] == ('32', '13', '7', 'M'))
    require(ok, f'R3 drag in 1002: got {got}')
    p.move(20, 9); p.move(22, 10)
    got = p.bytes()
    require(got == '', f'R3 motion without a button in 1002 sends nothing: got {got!r}')
    p.move(10, 5)
    p.bytes()
    for notch, cb in ((4, 64), (5, 65)):
        win.x('click', str(notch))
        got = p.bytes()
        require(got == sgr(cb, 10, 5), f'R5 one wheel notch ({notch}) in 1002: got {got!r}')
    mods = () if FAULT == 'shift' else ('shift',)
    p.click(7, 3, 1, mods, repeat=2)
    got = p.bytes()
    require(got == '', f'R6 Shift+double-click in 1002 sends the program nothing: got {got!r}')
    require(primary(win) == 'beta', f'R6 Shift+double-click selects "beta": PRIMARY {primary(win)!r}')
    p.stop()
    p.start('1003;1006')
    p.move(10, 5); p.bytes()
    p.move(13, 7)
    got = reports(p.bytes())
    require(got and all(g[0] == '35' for g in got) and got[-1] == ('35', '13', '7', 'M'), f'R4 motion in 1003: {got}')
    p.stop()


def wheel_story(win, p):
    p.start('1' if FAULT == 'wheel' else '1049;1')
    p.move(20, 8); p.bytes()
    n = max(1, math.ceil(p.rows / 10))
    for notch, key in ((5, 'OB'), (4, 'OA')):
        win.x('click', str(notch))
        got = p.bytes()
        require(got == (E + key) * n, f'W1 wheel {notch} on the alternate screen: got {got!r}, want {n} x ESC {key}')
    p.stop()
    p.start('-')
    p.move(20, 8)
    win.x('click', '4'); win.x('click', '5')
    got = p.bytes()
    require(got == '', f'W2 wheel on the normal screen sends nothing: got {got!r}')
    p.stop()


def select_story(win, p):
    p.start('2004')
    p.move(1, 3, .2)
    win.x('mousedown', '1'); time.sleep(.2)
    p.move(10, 3, .8)
    win.x('mouseup', '1'); time.sleep(.3)
    require(primary(win) == 'alpha beta', f'S1 drag selects "alpha beta": PRIMARY {primary(win)!r}')
    before = win.shot('select-before')
    p.click(30, 9)
    clear = win.shot('select-cleared')
    p.click(8, 3, repeat=1 if FAULT == 'select' else 2)
    require(primary(win) == 'beta', f'S2 double-click selects "beta": PRIMARY {primary(win)!r}')
    after = win.shot('select-beta')
    x0, y0 = p.at(7, 3, 0, 0); x1, y1 = p.at(10, 3, 1, 1)
    gx0, _ = p.at(12, 3, 0, 0); gx1, _ = p.at(16, 3, 1, 1)
    region = lambda im, a, b: [im.getpixel((x, y)) for y in range(y0 + 1, y1 - 1) for x in range(a + 1, b - 1)]
    require(region(after, x0, x1) != region(clear, x0, x1), 'S3 the selected "beta" cells are drawn highlighted')
    require(region(after, gx0, gx1) == region(clear, gx0, gx1), 'S3 unselected "gamma" cells are unchanged')
    require(region(before, x0, x1) != region(clear, x0, x1), 'S3 a single click elsewhere clears the selection')
    win.key('ctrl+shift+c')
    require(primary(win, 'clipboard') == 'beta', f'S4 Ctrl+Shift+C copies "beta": CLIPBOARD {primary(win, "clipboard")!r}')
    p.click(24, 3, repeat=2)
    require(primary(win) == '/srv/devops/x-y.txt', f'S2 double-click selects the path: PRIMARY {primary(win)!r}')
    p.click(4, 3, repeat=3)
    got = primary(win) or ''
    require(got.rstrip('\n') == LINE and got.count('\n') <= 1, f'S2 triple-click selects the line: PRIMARY {got!r}')
    got = p.bytes()
    require(got == '', f'S2 selecting sends the program nothing: got {got!r}')
    set_clip(win, 'PASTE-ME', 'clipboard')
    win.key('ctrl+shift+v')
    got = p.bytes()
    require(got == f'{E}[200~PASTE-ME{E}[201~', f'S4 Ctrl+Shift+V bracketed paste: got {got!r}')
    p.stop()
    p.start('-')
    set_clip(win, 'MID-TEXT', 'primary')
    p.click(20, 8, 2)
    got = p.bytes()
    require(got == 'MID-TEXT', f'S5 middle click pastes PRIMARY: got {got!r}')
    p.stop()


def mc_story(win, p):
    win.type('clear; /usr/bin/mc -u\n')
    until(lambda: '10Quit' in win.screen(), 'M1 mc started')
    time.sleep(1.5)
    rows = win.o.call(4, 'screen', tab_id=win.tab, history_offset=0, history_limit=0)['lines']
    rows = [l if isinstance(l, str) else l.get('text', '') for l in rows]
    top = next(i for i, l in enumerate(rows) if ' Left ' in l)
    p.click(rows[top].index('Left') + 2, top + 1)
    until(lambda: 'Listing format' in win.screen(), 'M1 click on mc "Left" opens its menu', 5)
    win.key('Escape', 'Escape')
    time.sleep(.8)
    bottom = next(i for i, l in enumerate(rows) if '10Quit' in l)
    row = bottom if FAULT == 'mc' else bottom + 1
    p.click(rows[bottom].index('10Quit') + 4, row)
    until(lambda: '10Quit' not in win.screen(), 'M1 click on mc "Quit" exits mc', 6)


def zoom_and_idle(win, p):
    cw, _ = p.geo[2], p.geo[3]
    p.start('-')
    p.move(20, 8)
    win.x('keydown', 'ctrl'); win.x('click', '4'); win.x('keyup', 'ctrl')
    time.sleep(1)
    p.stop()
    wider, _ = p.measure()
    require(wider > cw + .5, f'W3 Ctrl+wheel up zooms in: cell {cw:.2f} -> {wider:.2f} px')
    win.key('ctrl+0')
    time.sleep(1)
    p.measure()
    for col in range(5, 40, 5):
        p.move(col, 6)
    stop = time.time()
    time.sleep(4.5)
    late = [t for t in win.frames() if stop + 1 < t < stop + 4]
    require(not late, f'I1 0 frames 1-4 s after the pointer stops: {len(late)}')


def main():
    for tool in ('Xvfb', 'xdotool', 'xclip', 'ffmpeg', '/usr/bin/mc'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    win = Window('mouse', theme='dark', theme_dark='ink', font_family='IBM Plex Mono', font_size_pt=11, cell_height_scale=1.25,
                 cursor_blink='off')
    try:
        p = Pane(win)
        p.measure()
        report_story(win, p)
        wheel_story(win, p)
        select_story(win, p)
        mc_story(win, p)
        zoom_and_idle(win, p)
    finally:
        win.close()
    print('PASS: mouse')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: {e}')
        sys.exit(1)
