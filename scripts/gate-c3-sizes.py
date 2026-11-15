#!/usr/bin/env python3
"""C3 gate: comfortable sizes, HiDPI, and the look and feel of Ptyxis (design 2026-11-14, milestone C3; Marc: "Matching
the look and feel of Ptyxis would be great", his R1 yes to the sizes below).

Usage: gate-c3-sizes.py [--fault upscale|line|idle|font|noscroll|nohover|scale] [--only ID,ID...] [--keep-going]
                        [--scales 1,2] [--live]
A fault changes the run's input on purpose; a correct build must then FAIL, at the check the fault names.
--only runs the windows and steps those check IDs need and judges only them (for showing that one check passes).
--keep-going prints every failed check instead of stopping at the first (a diagnosis; the run still FAILs).
--scales runs only the scales given (a diagnosis; the gate is the run over all four).
--live is the separate DP-2 run (native Wayland, scale 1.25); run it only under /srv/devops/scripts/dp2-lock.

Harness: the Q2 gate's window (scripts/gate-q2-input.py Win, itself the Q1 and T2 harness: free_display from :100 up,
the owner API, the frame trace, tesseract), a real Clanky window on a private Xvfb display, software Vulkan, TZ=UTC,
bash (SHELL=/bin/bash) with Clanky's own OSC 133 integration. Here the window is 1200 x 800 logical px (the size of
the Ptyxis measurement) at WINIT_X11_SCALE_FACTOR s, so 1200 s x 800 s physical px at (0,0) on a screen 200 px larger.
Every s in 1, 1.25, 1.6 and 2 gets the same window and the same checks; every expected size below is logical px and
is multiplied by s. The config is the defaults: only restore_layout, startup_animation, history and animations off and
cursor_blink "off" are set (no font, size or theme key), so the default theme (Stone dark) and the default fonts are
what is judged. The gate's ~/.bashrc sources /etc/bashrc, puts ~/bin first on PATH, sets PS1='$ ' and cd's to
~/Kiln, so every tab is titled "Kiln". Screenshots leave out the pointer.

Sources (each expected value below names its source):
  [R1] Marc's R1, 2026-11-14 (the approved defaults at scale 1): terminal text 11 pt (14.67 px) with line height
       x 1.25; interface text 14 px; buttons, chips, tabs >= 32 px tall; title bar 46 px; 8 px gap between panes;
       14 px padding inside each pane. HiDPI: every size x the display scale (+-1 px), text rasterised at full
       resolution (glyph edge sharpness at 2 not lower than at 1, no upscaled bitmap), 1 px lines stay crisp,
       0 idle frames. Where Ptyxis differs, R1 wins.
  [P]  Ptyxis 50.1 measured by running it with Marc's settings (notes/ptyxis-measure.md, 2026-11-14; pixels only,
       no Ptyxis code): header 46; tabs 34 tall with 6 px above and below, radius about 9, 6 px apart, active fill
       white at 8 % over the bar, hovered 5 %, an 8 x 8 px close X only on the active or hovered tab, labels centred;
       header icon buttons 34 x 34, radius about 8, hover fill white at 6-10 %; search entry 34; menu rows 32 with a
       highlight of radius about 6; other controls 32 (chips, toggle, tool tabs); no hard line under the header (none,
       or a 4 px soft shade); overlay scrollbar 8 px wide, 7 px from the right edge, radius 4, the foreground at about
       15 % over the background, shown after a scroll then faded; keyboard focus a 2 px ring; scale 2 = exactly x 2.
       Line pitch = floor(natural font height x 1.25): IBM Plex Mono 11 gives 21 -> 26 there.
  [F]  the bundled font files assets/fonts/Geist-*.ttf and GeistMono-Regular.ttf, read with fontTools and rendered
       with FreeType through PIL (raqm layout), never through Clanky. Natural height = ceil(hhea ascent x px / upm)
       + ceil(-hhea descent x px / upm): the rounding that gives Ptyxis's 21 for IBM Plex Mono at 14.67 px from that
       font's own hhea (1025 / -275: 16 + 5 = 21; the gate re-derives it when the font is installed).
       Geist Mono (upm 1000, hhea 1005 / -295, advance 600) at 14.67 px: 15 + 5 = 20, so the cell is 8.8 x
       floor(20 x 1.25) = 25 px. "Kiln" in Geist Mono is 711 units tall: 10.43 px.
       Interface text 14 px [R1]: the ink width and height of "Kiln" and "Recent" rendered by FreeType from
       Geist-Regular, -Medium and -SemiBold at 14 s px (any of the three weights is accepted).
  [Q1] the approved mockup (Q1 gate): the tools panel is 236 px wide.
  [T]  assets/themes/stone-dark.json, the default dark theme: background #141210, foreground #ebe6df. The bar colour
       is measured (the modal colour of the title bar), so fills are judged against what is drawn under them.

Measurement. bar: the modal colour of the title bar row at y = 10 s over x 40-70 % of the width; the bar height is
the first row from y = 2 s down whose modal colour there differs from it by more than 1. A "lone line" is a row
whose modal colour differs by more than 6 from both its neighbours. Ink: pixels farther from the local background
than 35 % of the farthest one; the ink box of a word is the cluster of ink columns (gaps under 3 s px) under the
word tesseract found. A control's extent: along the label's centre column, the farthest row within 16 s px above
the ink top (and below the ink bottom) that differs by more than 2 from the colour around the control. A fill's
box: the 4-connected pixels within 4 of the colour sampled 4 s px above the label's ink. A hover's box: the
connected pixels that change (any channel >= 3) between the pointer away and the pointer on the control. Corner
radius: fitted to the pixels missing from the box's four corners (a circle quarter per corner).
Every tolerance is +-1 px (on base x s, which need not be whole) unless the line says otherwise.

Expected results, fixed before the work, at every s (logical px x s):
  D1 title bar height 46 [R1, P].
  D2 no hard line under the title bar: no lone line from the bar's last row to 6 s px below it [P].
  I1 0 frames from 1 s to 3.5 s after the window settles [R1].
  L1 crisp lines: the status line's top hairline and the command input's hairline (the two lowest lone lines in
     x 45-70 %) are 1..ceil(s) rows thick and every row of each keeps >= 85 % of its contrast at scale 1 [R1].
  U1 interface text 14 px [R1]: the tools footer's "Recent" and the active tab's "Kiln": ink width within 1 px and
     ink height within 1.5 px of the FreeType ink of the same word at 14 s px [F] (1.5: a whole-pixel ink box against
     a fractional expectation; the width is the finer measure: "Kiln" is 21.5-23.4 px at 14 px, 20 px at 13).
  F1 the tools footer's active segment "Files": extent 32 [P "tool tabs 32", R1 >= 32].
  F2 the command input's chips "bash" and "Agent access": extent 32 [P "chips 32", R1]; the Shell | Agent toggle
     ("Shell"): extent 32 [P "toggle 32", R1].
  F3 the title bar's search box ("Search ..."): extent 34 [P "search 34"].
  T1 with three tabs (Ctrl+Shift+T twice), the active (third) tab [P]: fill height 34, top 6 (centred in the bar),
     corner radius 9 +-2.5, fill = bar + 8 % x (255 - bar) per channel +-3; its label's ink centre within 4 s px of
     the fill's centre; a close X right of the label: an ink cluster 8 +-2.5 px wide and tall.
  T2 the pointer on the middle tab [P]: its fill = bar + 5 % x (255 - bar) +-3, height 34; the gap from its fill's
     right edge to the active fill's left edge is 6; it shows a close X (8 +-2.5); the first tab (neither active nor
     hovered) shows no ink between its label and the middle tab's fill.
  B1 header icon buttons [P]: the leftmost mark (tools toggle), the first mark right of the active tab (new tab "+")
     and the rightmost mark: each hover box is 34 x 34, radius 8 +-2.5, its fill (the commonest changed colour)
     between bar + 6 % and bar + 10 % of (255 - bar), +-3.
  C1 terminal text [R1, F]: a program on the alternate screen paints magenta cells at (1,1) and (21,11) and a yellow
     cell at the last column of the last row: cell width (x21 - x1) / 20 = 8.8, cell height (y11 - y1) / 10 = 25.
  C2 "Kiln" printed at row 5, column 3: ink height 10.43 +-1.5 [F].
  P1 padding 14 [R1]: cell (1,1)'s left edge is 14 right of the pane's left edge (the end of the run of pane
     background going left) and its top edge 14 below the pane's top (the end of that run going up; a soft shade of
     at most 4 s rows right under the title bar counts as pane); right of the yellow cell and below it the pane goes
     on for 14 .. 14 + one cell (+-1). Each walk starts 2 px off the painted cell: one pixel of the cell's own
     blended edge (a cell at a fractional position) is not the end of the pane.
  P2 the pane's left edge is at 236 + 8 = 244 [Q1, R1]: the 8 px gap lies between the tools panel and the pane.
  X1 sharp text [R1]: the mean width of the 15-85 % edge ramps of the terminal string "Kiln Hollow Mimic" and of the
     label "Recent" is at most the scale-1 value + 0.35 px; at s = 2, among 2 x 2 pixel blocks (all four alignments)
     that hold an edge pixel, at most 50 % are uniform (an upscaled bitmap is 100 %).
  S1 overlay scrollbar [P]: after `seq 1 400` and three wheel notches up, within 0.5 s: columns near the pane's
     right edge holding >= 30 s rows of bg + 15 % x (fg - bg) [T] (+-8) form one bar 8 wide whose right edge is 7
     from the pane's right edge, its top row narrower than its middle row (rounded ends).
  S2 3 s after the last notch the bar is gone, and 0 frames from 3.5 s to 6 s [P, R1].
  M1 the pane's right-click menu [P]: Down highlights the first enabled row, Down again the next one in the same
     group: the highlight moves by 32 (the row pitch); the highlight is 26 .. 32 tall, radius 6 +-2.5.
  K1 keyboard focus [P]: in Settings (Ctrl+,) one Tab press draws a ring: the largest changed box is at least
     16 x 16, and its changed band is 2 thick (+-1) at the middle of its left edge and of its top edge.
  G1 Split right (palette): the two panes are 8 apart along the middle row (the run between the left pane's colour
     and the right pane's colour) [R1].
  Live (--live, DP-2, native Wayland, scale 1.25, not part of the default run): D1, C1 and P1 on a window of
  Marc's own display, captured with grim; the window is closed after.

Planted faults (the check that must catch each): upscale (X1: the scale-2 text is the scale-1 text upscaled 2x
bilinear), line (L1: the scale-2 hairlines are blurred over two rows), idle (I1: a running "sleep 20"), font (C1:
font_size 13, today's size), noscroll (S1: no wheel notch), nohover (B1: the pointer is not moved onto the buttons),
scale (D1: the scale-2 window is started at WINIT_X11_SCALE_FACTOR 1).
"""
import importlib.util, json, math, os, re, shutil, subprocess, sys, time
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_C3_WORK', '/srv/devops/build/clanky-c3-gate-run'))
os.environ['CLANKY_Q2_WORK'] = str(ROOT)
ARGS = sys.argv[1:]
FAULT = ARGS[ARGS.index('--fault') + 1] if '--fault' in ARGS else None
ONLY = set(ARGS[ARGS.index('--only') + 1].split(',')) if '--only' in ARGS else None
LIVE = '--live' in ARGS
KEEP = '--keep-going' in ARGS
FAILED = []
sys.argv = sys.argv[:1]  # the harness modules read their own --fault from argv; they get none
spec = importlib.util.spec_from_file_location('q2', Path(__file__).with_name('gate-q2-input.py'))
q2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q2)
q1, gm, t2 = q2.q1, q2.gm, q2.t2
Fail, require, until = q1.Fail, q1.require, q1.until
import numpy as np
from PIL import Image, ImageDraw, ImageFont

REPO = Path(__file__).resolve().parents[1]
FONTS = REPO / 'assets/fonts'
LW, LH = 1200, 800
SCALES = (1, 1.25, 1.6, 2)
THEME = json.loads((REPO / 'assets/themes/stone-dark.json').read_text())
MAG, YEL = (255, 0, 255), (255, 255, 0)
TERM_TEXT = 'Kiln Hollow Mimic'
CFG = dict(restore_layout=False, startup_animation=False, history=False, animations=False, cursor_blink='off')
BASHRC = '[ -f /etc/bashrc ] && . /etc/bashrc\nPATH="$HOME/bin:$PATH"\nPS1=\'$ \'\ncd ~/Kiln\n'
PROG = r'''#!/usr/bin/python3 -I
# c3prog LOG: alternate screen, cursor hidden; magenta cells (1,1) and (21,11), yellow at the last cell,
# the text at row 5 column 3; writes "rows cols" to LOG.size; quits on "q".
import os, select, sys, termios, tty
fd = 0
old = termios.tcgetattr(fd)
tty.setraw(fd)
w = lambda s: os.write(1, s.encode())
try:
    w("\x1b[?1049h\x1b[?25l\x1b[2J")
    cols, rows = os.get_terminal_size()
    cell = lambda c, r, rgb: w(f"\x1b[{r};{c}H\x1b[48;2;{rgb}m \x1b[0m")
    cell(1, 1, "255;0;255"); cell(21, 11, "255;0;255"); cell(cols, rows, "255;255;0")
    w("\x1b[5;3H" + sys.argv[2])
    open(sys.argv[1] + ".size", "w").write(f"{rows} {cols}")
    while True:
        r, _, _ = select.select([fd], [], [], 120)
        if not r or os.read(fd, 64) == b"q":
            break
finally:
    w("\x1b[?25h\x1b[?1049l")
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
'''
RESULTS = {}   # per scale: measurements other scales compare with (L1, X1)
CHECK_STEPS = {  # which steps each check needs
    'D1': {'top'}, 'D2': {'top'}, 'L1': {'top'}, 'I1': {'idle'}, 'U1': {'footer', 'tabs'}, 'F1': {'footer'},
    'F2': {'input'}, 'F3': {'search'}, 'T1': {'tabs'}, 'T2': {'tabs'}, 'B1': {'tabs', 'buttons'},
    'C1': {'grid'}, 'C2': {'grid'}, 'P1': {'grid'}, 'P2': {'grid'}, 'X1': {'grid', 'footer'},
    'S1': {'grid', 'scroll'}, 'S2': {'grid', 'scroll'}, 'M1': {'menu'}, 'K1': {'focus'}, 'G1': {'split'},
}
FAULT_CHECK = {'upscale': 'X1', 'line': 'L1', 'idle': 'I1', 'font': 'C1', 'noscroll': 'S1', 'nohover': 'B1',
               'scale': 'D1'}
FAULT_SCALES = {'upscale': (1, 2), 'line': (1, 2), 'idle': (1,), 'font': (1,), 'noscroll': (1,), 'nohover': (1,),
                'scale': (2,)}
if FAULT:
    ONLY = {FAULT_CHECK.get(FAULT, '?')}


def want(cid):
    return ONLY is None or cid in ONLY


def check(cid, s, ok, text):
    """Judges check cid at scale s: a failure stops the gate only when the check is being judged."""
    if want(cid):
        if not ok and KEEP:
            FAILED.append(f'{cid} at scale {s}: {text}')
            print(f'FAIL (kept going): {cid} at scale {s}: {text}')
            return
        require(ok, f'{cid} at scale {s}: {text}')
        print(f'ok: {cid} at scale {s}: {text}')


def steps_needed():
    ids = CHECK_STEPS if ONLY is None else {k: v for k, v in CHECK_STEPS.items() if k in ONLY}
    return set().union(*ids.values()) if ids else set()


# ---------------------------------------------------------------- expected values from the font files [F]

def natural_height(path, px):
    from fontTools.ttLib import TTFont
    t = TTFont(str(path))
    upm, h = t['head'].unitsPerEm, t['hhea']
    return math.ceil(h.ascent * px / upm) + math.ceil(-h.descent * px / upm)


def mono_advance(path, px):
    from fontTools.ttLib import TTFont
    t = TTFont(str(path))
    return t['hmtx'][t.getBestCmap()[ord('0')]][0] * px / t['head'].unitsPerEm


def ft_ink(path, word, px):
    """(width, height) of word's ink rendered by FreeType at px, measured at 100 x px and scaled down."""
    big = px * 100
    font = ImageFont.truetype(str(path), int(round(big)))
    im = Image.new('L', (int(big * (len(word) + 2)), int(big * 2)))
    ImageDraw.Draw(im).text((big // 2, big // 2), word, font=font, fill=255)
    a = np.asarray(im) >= 128
    ys, xs = np.where(a)
    return (xs.max() - xs.min() + 1) * px / big, (ys.max() - ys.min() + 1) * px / big


TERM_PX = 11 * 96 / 72                      # 11 pt [R1] = 14.67 px
NAT = natural_height(FONTS / 'GeistMono-Regular.ttf', TERM_PX)
CELL_W = mono_advance(FONTS / 'GeistMono-Regular.ttf', TERM_PX)
CELL_H = math.floor(NAT * 1.25)


def ui_ink(word, s):
    """Range of (width, height) over the three bundled Geist weights at 14 s px."""
    got = [ft_ink(FONTS / f'Geist-{w}.ttf', word, 14 * s) for w in ('Regular', 'Medium', 'SemiBold')]
    return (min(g[0] for g in got), max(g[0] for g in got)), (min(g[1] for g in got), max(g[1] for g in got))


# ---------------------------------------------------------------- the window

def make_home(home):
    home.mkdir(mode=0o700, exist_ok=True)
    (home / '.bashrc').write_text(BASHRC)
    (home / 'Kiln').mkdir(exist_ok=True)
    b = home / 'bin'
    b.mkdir(exist_ok=True)
    (b / 'c3prog').write_text(PROG)
    (b / 'c3prog').chmod(0o755)


q2.make_home = make_home


class Win(q2.Win):
    """The Q2 window at WINIT_X11_SCALE_FACTOR `scale`, LW x LH logical px, screenshots of the whole window."""

    def __init__(self, name, cfg, scale, env_scale=None):
        self.scale, self.env_scale = scale, env_scale or scale
        self.pw, self.ph = round(LW * scale), round(LH * scale)
        super().__init__(name, cfg)

    def _start(self, cfg, extra_env):
        real = subprocess.Popen
        screen = f'{self.pw + 200}x{self.ph + 200}x24'
        es = str(self.env_scale)

        def popen(args, **kw):
            if args and args[0] == 'Xvfb':
                args = [screen if re.fullmatch(r'\d+x\d+x24', str(a)) else a for a in args]
            elif args and str(args[0]) == str(q2.APP) and kw.get('env') is not None:
                kw['env'] = dict(kw['env'], WINIT_X11_SCALE_FACTOR=es)
            return real(args, **kw)
        old = q2.W, q2.H
        q2.W, q2.H = self.pw, self.ph
        subprocess.Popen = popen
        try:
            super()._start(cfg, extra_env)
        finally:
            subprocess.Popen = real
            q2.W, q2.H = old

    def shot(self, name):
        path = self.w / f'{name}.png'
        subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-f', 'x11grab', '-draw_mouse', '0', '-video_size',
                        f'{self.pw}x{self.ph}', '-i', self.env['DISPLAY'] + '+0,0', '-frames:v', '1', str(path)],
                       env=self.env, check=True, stdin=subprocess.DEVNULL)
        return Image.open(path).convert('RGB')

    def away(self):
        self.x('mousemove', '--window', self.wid, str(int(100 * self.scale)), str(int(self.ph * .6)))
        time.sleep(.4)

    def move(self, x, y):
        self.x('mousemove', '--window', self.wid, str(int(x)), str(int(y)))
        time.sleep(.6)


# ---------------------------------------------------------------- pixels

def A(im):
    return np.asarray(im).astype(int)


def cdiff(a, b):
    return max(abs(int(x) - int(y)) for x, y in zip(a, b))


def mode_colour(a, y, x0, x1):
    row = a[y, x0:x1]
    keys, counts = np.unique(row.reshape(-1, 3), axis=0, return_counts=True)
    return tuple(int(v) for v in keys[counts.argmax()])


def mix(base, frac, top=(255, 255, 255)):
    return tuple(b + frac * (t - b) for b, t in zip(base, top))


def near_f(c, want, tol):
    return all(abs(x - y) <= tol for x, y in zip(c, want))


def title_bar(a, s):
    h, w = a.shape[:2]
    x0, x1 = int(.40 * w), int(.70 * w)
    bar = mode_colour(a, int(10 * s), x0, x1)
    y = int(2 * s)
    while y < h and cdiff(mode_colour(a, y, x0, x1), bar) <= 1:
        y += 1
    return bar, y


def lone_lines(a, y0, y1, x0, x1):
    """Runs of 1-3 rows in [y0, y1) whose modal colours differ by > 6 from the rows just outside the run."""
    rows = [mode_colour(a, y, x0, x1) for y in range(max(0, y0 - 1), min(a.shape[0], y1 + 4))]
    y0 = max(1, y0)
    out, y = [], 1
    while y < len(rows) - 4:
        for n in (1, 2, 3):
            above, below = rows[y - 1], rows[y + n]
            if all(cdiff(rows[y + k], above) > 6 and cdiff(rows[y + k], below) > 6 for k in range(n)):
                out.append((y0 - 1 + y, n, [rows[y + k] for k in range(n)], above, below))
                y += n
                break
        else:
            y += 1
    return out


def ink_mask(a, box, bg):
    x0, y0, x1, y1 = box
    d = np.abs(a[y0:y1, x0:x1] - np.array(bg)).max(axis=2)
    return d >= max(12, .35 * d.max())


def ink_box(a, word, bg, s):
    """Ink box (x0, y0, x1, y1 inclusive) of the column cluster under the OCR word's centre."""
    h, w = a.shape[:2]
    pad = int(3 * s)
    box = (max(0, int(word[1]) - pad), max(0, int(word[2]) - pad), min(w, int(word[3]) + pad + 1),
           min(h, int(word[4]) + pad + 1))
    m = ink_mask(a, box, bg)
    cols = np.where(m.any(axis=0))[0]
    require(len(cols), f'ink under the word {word[0]!r}')
    clusters = [[cols[0], cols[0]]]
    for c in cols[1:]:
        if c - clusters[-1][1] <= 3 * s + 1:
            clusters[-1][1] = c
        else:
            clusters.append([c, c])
    cx = (word[1] + word[3]) / 2 - box[0]
    best = min(clusters, key=lambda k: 0 if k[0] <= cx <= k[1] else min(abs(cx - k[0]), abs(cx - k[1])))
    sub = m[:, best[0]:best[1] + 1]
    ys = np.where(sub.any(axis=1))[0]
    return box[0] + best[0], box[1] + ys[0], box[0] + best[1], box[1] + ys[-1]


def extent(a, ink, bg, s):
    """Top and bottom rows of the control around the ink box, along the ink's centre column: starting 2 s px off
    the ink, the first band of rows that differ from bg (the control's fill, or its border when the inside is bg);
    its far edge. Nothing within 16 s px: the ink's own edge."""
    cx = (ink[0] + ink[2]) // 2
    off = math.ceil(2 * s)

    def walk(start, step, limit):
        edge, seen = None, False
        for y in range(start, limit, step):
            if not 0 <= y < a.shape[0]:
                break
            if cdiff(a[y, cx], bg) > 2:
                edge, seen = y, True
            elif seen:
                break
        return edge
    top = walk(ink[1] - off, -1, ink[1] - int(16 * s) - 1)
    bot = walk(ink[3] + off, 1, ink[3] + int(16 * s) + 1)
    return (ink[1] if top is None else top), (ink[3] if bot is None else bot)


def changed_box(dm, seed, s):
    """Box and mask of the changed pixels connected (through gaps of up to 2 s px) to the seed or nearest to it."""
    from scipy import ndimage
    grown = ndimage.binary_dilation(dm, iterations=max(1, round(2 * s)))
    lab, n = ndimage.label(grown)
    if not n:
        return None, None
    v = lab[seed[1], seed[0]]
    if not v:
        ys, xs = np.where(lab > 0)
        i = int(np.argmin((xs - seed[0]) ** 2 + (ys - seed[1]) ** 2))
        v = lab[ys[i], xs[i]]
    m = (lab == v) & dm
    ys, xs = np.where(m)
    return (xs.min(), ys.min(), xs.max(), ys.max()), m


def component(mask, seed):
    """Bounding box and mask of the 4-connected True region containing seed (x, y)."""
    from scipy import ndimage
    lab, _ = ndimage.label(mask)
    v = lab[seed[1], seed[0]]
    require(v, f'a region at {seed}')
    m = lab == v
    ys, xs = np.where(m)
    return (xs.min(), ys.min(), xs.max(), ys.max()), m


def fill_box(a, seed, under, tol=4):
    """The fill around seed: its colour, its box of pixels within tol of that colour (4-connected), the coverage
    map against `under`, the box of the covered region (partly covered edge pixels included) and its height with
    the edge rows counted by their coverage (along the box's centre column)."""
    c = tuple(int(v) for v in a[seed[1], seed[0]])
    box, _ = component(np.abs(a - np.array(c)).max(axis=2) <= tol, seed)
    cov = coverage(a, under, c)
    cbox, _ = component(cov > .05, seed)
    xc = (box[0] + box[2]) // 2
    h = box[3] - box[1] + 1 + (cov[box[1] - 1, xc] if box[1] > 0 else 0) + \
        (cov[box[3] + 1, xc] if box[3] + 1 < a.shape[0] else 0)
    return dict(colour=c, box=box, cov=cov, cbox=cbox, h=float(h))


def coverage(img, under, fill):
    """Per pixel, how much of `fill` covers `under` (a colour or an image of the same shape), 0..1, read in the
    channel where fill and under differ most. Text brighter than the fill counts as covered."""
    under = np.broadcast_to(np.array(under), img.shape) if np.ndim(under) == 1 else under
    fill = np.array(fill)
    k = int(np.argmax(np.abs(fill - np.median(under.reshape(-1, 3), axis=0))))
    den = (fill[k] - under[..., k]).astype(float)
    ok = np.abs(den) >= 1
    out = np.zeros(img.shape[:2])
    out[ok] = (img[..., k][ok] - under[..., k][ok]) / den[ok]
    return np.clip(out, 0, 1)


def radius(cov, box, expect):
    """Corner radius from the area missing in the box's corners: per corner, the uncovered area of a square of side
    1.25 x the expected radius + 1 (at most half the box; small enough to keep a label out of it), less the straight
    edges' own partial rows, is r^2 (1 - pi/4). The mean of the middle two of the four corners."""
    x0, y0, x1, y1 = box
    sub = cov[y0:y1 + 1, x0:x1 + 1]
    hh, ww = sub.shape
    R0 = max(2, min(min(hh, ww) // 2, round(1.25 * expect) + 1))
    ct, cb = sub[0, ww // 2], sub[-1, ww // 2]
    cl, cr = sub[hh // 2, 0], sub[hh // 2, -1]
    rs = []
    for q, ey, ex in ((sub[:R0, :R0], ct, cl), (sub[:R0, -R0:][:, ::-1], ct, cr),
                      (sub[-R0:, :R0][::-1], cb, cl), (sub[-R0:, -R0:][::-1, ::-1], cb, cr)):
        miss = float((1 - q).sum()) - R0 * (1 - ey) - R0 * (1 - ex) + (1 - ey) * (1 - ex)
        rs.append(math.sqrt(max(0.0, miss) / (1 - math.pi / 4)))
    rs.sort()
    return (rs[1] + rs[2]) / 2


def diff_mask(a, b, tol=3):
    return np.abs(a - b).max(axis=2) >= tol


def ocr(win, im, box, name):
    sc = max(2, round(3 / win.scale))
    return q1.ocr_words(im, tuple(int(v) for v in box), sc, 11)


def find(words, pred, label):
    hits = [w for w in words if pred(w[0])]
    require(hits, f'{label} (read {[w[0] for w in words][:30]})')
    return hits


def ramp_width(a, box, bg, fg_lum=None):
    """Mean number of 15-85 % pixels in each monotonic edge ramp, rows and columns, in box."""
    x0, y0, x1, y1 = box
    lum = a[y0:y1, x0:x1].mean(axis=2)
    bl = sum(bg) / 3
    top = fg_lum if fg_lum is not None else lum.max()
    t = (lum - bl) / max(1.0, top - bl)
    widths = []
    for arr in list(t) + list(t.T):
        i, n = 0, len(arr)
        while i < n - 1:
            if arr[i] < .15 or arr[i] > .85:
                lo = arr[i] < .15
                j = i + 1
                while j < n and .15 <= arr[j] <= .85:
                    j += 1
                if j < n and j > i + 1 and ((lo and arr[j] > .85) or (not lo and arr[j] < .15)):
                    widths.append(j - i - 1)
                elif j < n and j == i + 1 and ((lo and arr[j] > .85) or (not lo and arr[j] < .15)):
                    widths.append(0)
                i = j
            else:
                i += 1
    require(len(widths) >= 20, f'enough text edges to judge ({len(widths)})')
    return sum(widths) / len(widths)


def uniform_blocks(a, box, bg):
    x0, y0, x1, y1 = box
    lum = a[y0:y1, x0:x1].mean(axis=2)
    bl = sum(bg) / 3
    t = (lum - bl) / max(1.0, lum.max() - bl)
    worst = 0.0
    for oy in (0, 1):
        for ox in (0, 1):
            L, T = lum[oy:, ox:], t[oy:, ox:]
            hh, ww = (L.shape[0] // 2) * 2, (L.shape[1] // 2) * 2
            L, T = L[:hh, :ww], T[:hh, :ww]
            Lb = L.reshape(hh // 2, 2, ww // 2, 2)
            Tb = T.reshape(hh // 2, 2, ww // 2, 2)
            edge = ((Tb >= .15) & (Tb <= .85)).any(axis=(1, 3))
            uni = (Lb.max(axis=(1, 3)) - Lb.min(axis=(1, 3))) <= 6
            if edge.sum():
                worst = max(worst, float((edge & uni).sum() / edge.sum()))
    return worst


# ---------------------------------------------------------------- steps

def step_top(win, s, R):
    im = win.shot('top')
    a = A(im)
    bar, hgt = title_bar(a, s)
    R['bar'], R['bar_h'] = bar, hgt
    check('D1', s, abs(hgt - 46 * s) <= 1, f'title bar height {hgt} px, expected {46 * s:g} +-1 [R1, P] (bar {bar})')
    w = a.shape[1]
    x0, x1 = int(.40 * w), int(.70 * w)
    lines = lone_lines(a, hgt - 1, hgt + int(6 * s) + 1, x0, x1)
    check('D2', s, not lines, f'no hard line under the title bar (lone lines at {[l[0] for l in lines]}) [P]')
    xs0, xs1 = int(.45 * w), int(.70 * w)
    low = lone_lines(a, a.shape[0] - int(200 * s), a.shape[0] - 1, xs0, xs1)
    found = sorted(low, key=lambda l: -l[0])[:2]
    if FAULT == 'line' and s == 2:
        for y, n, *_ in found:
            band = a[y - 1:y + n + 1].copy()
            a[y - 1:y + n + 1] = (band + np.roll(band, 1, axis=0)) // 2
        low = lone_lines(a, a.shape[0] - int(200 * s), a.shape[0] - 1, xs0, xs1)
        lines_blur = []
        for y, n, *_ in found:
            rows = [mode_colour(a, yy, xs0, xs1) for yy in range(y - 2, y + n + 3)]
            lines_blur.append(rows)
        R['blur'] = lines_blur
    R['hair'] = []
    for y, n, rows, above, below in found:
        R['hair'].append((y, n, [cdiff(r, below) for r in rows]))
    if want('L1'):
        require(len(found) == 2, f'L1 at scale {s}: the status line and command input hairlines found '
                                 f'({[(l[0], l[1]) for l in low]})')
        if s == 1:
            print(f'ok: L1 at scale 1: hairlines {[(y, n, c) for y, n, c in R["hair"]]} measured as the reference')
        else:
            ref = RESULTS[1]['hair']
            if FAULT == 'line' and s == 2:
                # judged on the blurred rows: the run now spans the blurred rows, each with its own contrast
                hair = []
                for (y, n, *_), rows in zip(found, R['blur']):
                    below = rows[-1]
                    run = [cdiff(r, below) for r in rows[1:-1]]
                    run = [c for c in run if c > 6]
                    hair.append((y, len(run), run))
                R['hair'] = hair
            for (y, n, cs), (_, n1, c1) in zip(R['hair'], ref):
                full = max(c1)
                check('L1', s, 1 <= n <= math.ceil(s) and min(cs) >= .85 * full,
                      f'hairline at y {y}: {n} rows (1..{math.ceil(s)}), contrast {cs} >= 85 % of {full} at scale 1')


def step_idle(win, s, R):
    if FAULT == 'idle':
        win.type('sleep 20')
        win.key('Return')
        time.sleep(1.5)
        win.away()
    if want('I1'):
        q2.idle(win, f'I1 at scale {s}')
        print(f'ok: I1 at scale {s}: 0 idle frames')
    if FAULT == 'idle':
        win.key('ctrl+c')


def step_footer(win, s, R):
    im = win.shot('footer')
    a = A(im)
    h, w = a.shape[:2]
    panel = tuple(int(v) for v in a[int(h * .6), int(2 * s)])
    R['panel'] = panel
    words = ocr(win, im, (0, h - 140 * s, 236 * s, h), 'footer')
    rec = find(words, lambda t: 'recent' in t, f'U1 at scale {s}: the tools footer reads "Recent"')[0]
    fil = find(words, lambda t: 'files' in t, f'F1 at scale {s}: the tools footer reads "Files"')[0]
    ib = ink_box(a, rec, panel, s)
    (w0, w1), (h0, h1) = ui_ink('Recent', s)
    iw, ih = ib[2] - ib[0] + 1, ib[3] - ib[1] + 1
    check('U1', s, w0 - 1 <= iw <= w1 + 1 and h0 - 1.5 <= ih <= h1 + 1.5,
          f'"Recent" ink {iw} x {ih} px, expected {w0:.1f}-{w1:.1f} (+-1) x {h0:.1f}-{h1:.1f} (+-1.5) [R1, F]')
    R['recent_box'] = (ib[0] - 2, ib[1] - 2, ib[2] + 3, ib[3] + 3)
    R['recent_bg'] = panel
    R['footer_img'] = a
    fb = ink_box(a, fil, panel, s)
    top, bot = extent(a, fb, panel, s)
    check('F1', s, abs(bot - top + 1 - 32 * s) <= 1, f'"Files" segment {bot - top + 1} px tall, expected {32 * s:g} +-1 [P, R1]')


def step_input(win, s, R):
    im = win.shot('input')
    a = A(im)
    h, w = a.shape[:2]
    x0 = int(244 * s)
    words = ocr(win, im, (x0, h - 170 * s, w, h - 24 * s), 'input')
    for word, label in (('bash', 'chip "bash"'), ('access', 'chip "Agent access"'), ('shell', 'toggle "Shell"')):
        hit = find(words, lambda t, word=word: word in t, f'F2 at scale {s}: the command input reads {word!r}')
        wd = max(hit, key=lambda v: v[2])
        cy = int((wd[2] + wd[4]) / 2)
        bg = mode_colour(a, cy, x0, w)  # the input area around the controls
        ib = ink_box(a, wd, bg, s)
        top, bot = extent(a, ib, bg, s)
        check('F2', s, abs(bot - top + 1 - 32 * s) <= 1,
              f'{label} {bot - top + 1} px tall, expected {32 * s:g} +-1 [P, R1]')


def step_search(win, s, R):
    im = win.shot('search')
    a = A(im)
    h, w = a.shape[:2]
    bar = R.get('bar') or title_bar(a, s)[0]
    words = ocr(win, im, (w * .45, 0, w, 50 * s), 'search')
    wd = find(words, lambda t: 'search' in t, f'F3 at scale {s}: the title bar reads "Search"')[0]
    ib = ink_box(a, wd, bar, s)
    top, bot = extent(a, ib, bar, s)
    check('F3', s, abs(bot - top + 1 - 34 * s) <= 1, f'search box {bot - top + 1} px tall, expected {34 * s:g} +-1 [P]')


def tab_words(win, im, s):
    words = ocr(win, im, (0, 0, im.width * .75, 52 * s), 'tabs')
    return sorted([w for w in words if 'kil' in w[0] or 'iln' in w[0]], key=lambda w: w[1])


def x_mark(a, box, x_from, bg, s):
    """Ink box of the cluster right of x_from inside box (the close X), or None."""
    x0, y0, x1, y1 = box
    xa = int(x_from)
    if xa >= x1:
        return None
    d = np.abs(a[y0:y1 + 1, xa:x1 + 1] - np.array(bg)).max(axis=2) > 40
    cols = np.where(d.any(axis=0))[0]
    if not len(cols):
        return None
    c0 = cols[0]
    c1 = c0
    for c in cols[1:]:
        if c - c1 < 3 * s:
            c1 = c
        else:
            break
    ys = np.where(d[:, c0:c1 + 1].any(axis=1))[0]
    return c1 - c0 + 1, ys[-1] - ys[0] + 1


def step_tabs(win, s, R):
    before = set(win.terminals())
    for _ in range(2):
        win.key('ctrl+shift+t')
        time.sleep(1.5)
    until(lambda: len(win.terminals()) >= 3, f'setup at scale {s}: Ctrl+Shift+T twice gives three tabs', 15)
    new = [t for t in win.terminals() if t not in before]
    win.tab = new[-1]
    time.sleep(2)
    win.away()
    im = win.shot('tabs')
    a = A(im)
    bar = R['bar'] if 'bar' in R else title_bar(a, s)[0]
    R['bar'] = bar
    tw = tab_words(win, im, s)
    require(len(tw) >= 3, f'T1 at scale {s}: the title bar reads "Kiln" on three tabs ({len(tw)})')
    act = tw[-1]
    ib = ink_box(a, act, bar, s)
    seed = ((ib[0] + ib[2]) // 2, max(0, ib[1] - int(4 * s)))
    f = fill_box(a, seed, bar)
    box, colour = f['box'], f['colour']
    R['active_fill'] = box
    want_fill = mix(bar, .08)
    fh, top = f['h'], box[1]
    r = radius(f['cov'], f['cbox'], 9 * s)
    check('T1', s, abs(fh - 34 * s) <= 1, f'active tab fill {fh:.1f} px tall, expected {34 * s:g} +-1 [P]')
    check('T1', s, abs(top - 6 * s) <= 1, f'active tab top at y {top}, expected {6 * s:g} +-1 [P]')
    check('T1', s, abs(r - 9 * s) <= 2.5, f'active tab corner radius {r:g}, expected {9 * s:g} +-2.5 [P]')
    check('T1', s, near_f(colour, want_fill, 3), f'active tab fill {colour}, expected bar {bar} + 8 % = '
                                                  f'{tuple(round(v) for v in want_fill)} +-3 [P]')
    lab_c, fill_c = (ib[0] + ib[2]) / 2, (box[0] + box[2]) / 2
    check('T1', s, abs(lab_c - fill_c) <= 4 * s, f'label centre x {lab_c:g} vs fill centre {fill_c:g}, within {4 * s:g} [P]')
    (w0, w1), (h0, h1) = ui_ink('Kiln', s)
    iw, ih = ib[2] - ib[0] + 1, ib[3] - ib[1] + 1
    check('U1', s, w0 - 1 <= iw <= w1 + 1 and h0 - 1.5 <= ih <= h1 + 1.5,
          f'tab label "Kiln" ink {iw} x {ih} px, expected {w0:.1f}-{w1:.1f} (+-1) x {h0:.1f}-{h1:.1f} (+-1.5) [R1, F]')
    xm = x_mark(a, box, ib[2] + 2 * s, colour, s)
    check('T1', s, xm is not None and all(abs(v - 8 * s) <= 2.5 for v in xm),
          f'active tab close X {xm}, expected {8 * s:g} +-2.5 square [P]')
    # the pointer on the middle tab
    mid = tw[-2]
    if want('T2'):
        win.move((mid[1] + mid[3]) / 2, (mid[2] + mid[4]) / 2)
        im2 = win.shot('tab-hover')
        a2 = A(im2)
        mb = ink_box(a2, mid, bar, s)
        hf = fill_box(a2, ((mb[0] + mb[2]) // 2, max(0, mb[1] - int(4 * s))), bar)
        hbox, hcol = hf['box'], hf['colour']
        want_h = mix(bar, .05)
        check('T2', s, near_f(hcol, want_h, 3), f'hovered tab fill {hcol}, expected bar + 5 % = '
                                                 f'{tuple(round(v) for v in want_h)} +-3 [P]')
        check('T2', s, abs(hf['h'] - 34 * s) <= 1, f'hovered tab fill {hf["h"]:.1f} px tall, expected {34 * s:g} +-1 [P]')
        gap = box[0] - hbox[2] - 1
        check('T2', s, abs(gap - 6 * s) <= 1, f'gap between tabs {gap} px, expected {6 * s:g} +-1 [P]')
        hx = x_mark(a2, hbox, mb[2] + 2 * s, hcol, s)
        check('T2', s, hx is not None and all(abs(v - 8 * s) <= 2.5 for v in hx),
              f'hovered tab close X {hx}, expected {8 * s:g} +-2.5 [P]')
        first = tw[0]
        fb = ink_box(a2, first, bar, s)
        idle_x = x_mark(a2, (int(fb[2] + 2 * s), hbox[1], hbox[0] - 1, hbox[3]), fb[2] + 2 * s, bar, s)
        check('T2', s, idle_x is None, f'no close X on the idle first tab (ink {idle_x}) [P]')
        win.away()
    R['tabs_img'] = a


def step_buttons(win, s, R):
    a = R['tabs_img']
    bar = R['bar']
    hgt = int(46 * s)
    mk = q1.marks(Image.fromarray(a.astype(np.uint8)), int(4 * s), min(a.shape[0], hgt - int(4 * s)), 0,
                  a.shape[1], bar)
    require(len(mk) >= 3, f'B1 at scale {s}: marks in the title bar ({mk})')
    act_right = R['active_fill'][2]
    plus = next((m for m in mk if m[0] > act_right + 2), None)
    require(plus, f'B1 at scale {s}: a mark right of the active tab ({mk}, tab ends {act_right})')
    for label, m in (('tools toggle', mk[0]), ('new tab +', plus), ('rightmost button', mk[-1])):
        cx = (m[0] + m[1]) // 2
        ys = [y for y in range(0, hgt) if cdiff(a[y, cx], bar) > 40]
        cy = (ys[0] + ys[-1]) // 2 if ys else hgt // 2
        if FAULT != 'nohover':
            win.move(cx, cy)
        im = win.shot('hover-' + label.split()[0])
        b = A(im)
        dm = diff_mask(a, b)
        box, mm = changed_box(dm, (cx, cy), s)
        if box is None or abs((box[0] + box[2]) / 2 - cx) > 40 * s:
            check('B1', s, False, f'{label}: the pointer on it changes the button (no change near x {cx}) [P]')
            win.away()
            continue
        bw, bh = box[2] - box[0] + 1, box[3] - box[1] + 1
        changed = b[mm]
        if len(changed):
            keys, counts = np.unique(changed.reshape(-1, 3), axis=0, return_counts=True)
            fill = tuple(int(v) for v in keys[counts.argmax()])
        else:
            fill = None
        lo, hi = mix(bar, .06), mix(bar, .10)
        check('B1', s, abs(bw - 34 * s) <= 1 and abs(bh - 34 * s) <= 1,
              f'{label} hover box {bw} x {bh}, expected {34 * s:g} square +-1 [P]')
        r = radius(coverage(b, a, fill), box, 8 * s) if fill else 0
        check('B1', s, abs(r - 8 * s) <= 2.5, f'{label} hover radius {r:g}, expected {8 * s:g} +-2.5 [P]')
        check('B1', s, fill is not None and all(l - 3 <= f <= u + 3 for f, l, u in zip(fill, lo, hi)),
              f'{label} hover fill {fill}, expected between bar + 6 % and + 10 % '
              f'({tuple(round(v) for v in lo)}..{tuple(round(v) for v in hi)}) +-3 [P]')
        win.away()


def step_grid(win, s, R):
    log = win.w / 'c3prog.log'
    win.type(f'c3prog {log} "{TERM_TEXT}"')
    win.key('Return')
    size = Path(str(log) + '.size')
    until(lambda: size.exists() and size.read_text().strip(), f'setup at scale {s}: the alternate-screen program runs', 15)
    rows, cols = map(int, size.read_text().split())
    time.sleep(1.2)
    win.away()
    im = win.shot('grid')
    a = A(im)
    h, w = a.shape[:2]

    def cells(colour):
        mask = np.abs(a - np.array(colour)).max(axis=2) <= 2
        from scipy import ndimage
        lab, n = ndimage.label(mask)
        out = []
        for i in range(1, n + 1):
            ys, xs = np.where(lab == i)
            if len(xs) > 20:
                out.append((xs.min(), ys.min(), xs.max(), ys.max()))
        return sorted(out, key=lambda b: (b[1], b[0]))
    mg, yl = cells(MAG), cells(YEL)
    require(len(mg) == 2 and len(yl) == 1, f'C1 at scale {s}: two magenta cells and one yellow cell ({mg}, {yl})')
    (ax, ay, *_), (bx, by, *_) = mg
    cw, ch = (bx - ax) / 20, (by - ay) / 10
    R['geo'] = (ax, ay, cw, ch)
    check('C1', s, abs(cw - CELL_W * s) <= 1 and abs(ch - CELL_H * s) <= 1,
          f'cell {cw:g} x {ch:g} px, expected {CELL_W * s:g} x {CELL_H * s:g} +-1 [R1, F]')
    pane_bg = tuple(int(v) for v in a[int(ay + 8.5 * ch), int(ax + 12.5 * cw)])
    R['pane_bg'] = pane_bg
    # C2: "Kiln" at row 5 column 3
    kx0, ky0 = int(ax + 2 * cw), int(ay + 4 * ch)
    kbox = (kx0, ky0, int(kx0 + 4 * cw), int(ky0 + ch))
    km = ink_mask(a, kbox, pane_bg)
    kys = np.where(km.any(axis=1))[0]
    kh = kys[-1] - kys[0] + 1 if len(kys) else 0
    want_h = 711 * TERM_PX / 1000 * s
    check('C2', s, abs(kh - want_h) <= 1.5, f'"Kiln" ink height {kh} px, expected {want_h:.2f} +-1.5 [F]')
    R['term_box'] = (kx0 - 1, ky0, int(ax + (2 + len(TERM_TEXT)) * cw) + 1, int(ky0 + ch))
    R['grid_img'] = a
    # P1, P2
    yrow = int(ay + ch / 2)
    x = ax - 2  # one pixel of the cell's own blended edge is skipped (fractional cell positions)
    while x > 0 and cdiff(a[yrow, x], pane_bg) <= 2:
        x -= 1
    pane_left = x + 1
    xc = int(ax + 1.5 * cw)
    y = ay - 2
    while y > 0 and cdiff(a[y, xc], pane_bg) <= 2:
        y -= 1
    run_top = y + 1
    bar_h = R.get('bar_h') or title_bar(a, s)[1]
    pane_top = bar_h if 0 <= run_top - bar_h <= math.ceil(4 * s) else run_top
    yx1, yy1 = yl[0][2], yl[0][3]
    x = yx1 + 2
    while x < w - 1 and cdiff(a[int((yl[0][1] + yy1) / 2), x], pane_bg) <= 2:
        x += 1
    pane_right = x - 1
    y = yy1 + 2
    xc2 = int((yl[0][0] + yx1) / 2)
    while y < h - 1 and cdiff(a[y, xc2], pane_bg) <= 2:
        y += 1
    pane_bottom = y - 1
    R['pane'] = (pane_left, pane_top, pane_right, pane_bottom)
    lp, tp = ax - pane_left, ay - pane_top
    rp, bp = pane_right - yx1, pane_bottom - yy1
    check('P1', s, abs(lp - 14 * s) <= 1 and abs(tp - 14 * s) <= 1,
          f'padding left {lp}, top {tp} px, expected {14 * s:g} +-1 [R1]')
    check('P1', s, 14 * s - 1 <= rp <= 14 * s + cw + 1 and 14 * s - 1 <= bp <= 14 * s + ch + 1,
          f'padding right {rp}, bottom {bp} px, expected {14 * s:g} .. {14 * s:g} + one cell, +-1 [R1]')
    check('P2', s, abs(pane_left - 244 * s) <= 1, f'pane left edge at x {pane_left}, expected (236 + 8) x {s} = '
                                                   f'{244 * s:g} +-1 [Q1, R1]')
    win.type('q')
    time.sleep(1)


def step_sharp(win, s, R):
    if not want('X1'):
        return
    a = R['grid_img']
    tb = R['term_box']
    if FAULT == 'upscale' and s == 2:
        r1 = RESULTS[1]
        t1 = r1['term_box']
        crop = Image.fromarray(r1['grid_img'][t1[1]:t1[3], t1[0]:t1[2]].astype(np.uint8))
        up = A(crop.resize((crop.width * 2, crop.height * 2), Image.BILINEAR))
        a = a.copy()
        hh, ww = min(up.shape[0], a.shape[0] - tb[1]), min(up.shape[1], a.shape[1] - tb[0])
        tb = (tb[0], tb[1], tb[0] + ww, tb[1] + hh)
        a[tb[1]:tb[3], tb[0]:tb[2]] = up[:hh, :ww]
    rt = ramp_width(a, tb, R['pane_bg'])
    rr = ramp_width(R['footer_img'], R['recent_box'], R['recent_bg'])
    R['ramp'] = (rt, rr)
    if s == 1:
        print(f'ok: X1 at scale 1: edge ramps {rt:.2f} (terminal), {rr:.2f} (label) measured as the reference')
        return
    t1, l1 = RESULTS[1]['ramp']
    check('X1', s, rt <= t1 + .35 and rr <= l1 + .35,
          f'edge ramps {rt:.2f} (terminal) and {rr:.2f} (label) px, at most scale 1\'s {t1:.2f} and {l1:.2f} + 0.35 [R1]')
    if s == 2:
        u = uniform_blocks(a, tb, R['pane_bg'])
        check('X1', s, u <= .5, f'{u:.0%} of edge 2 x 2 blocks uniform, at most 50 % (no upscaled bitmap) [R1]')


def bar_columns(a, pane, colour, s, y0, y1):
    x0 = int(pane[2] - 30 * s)
    out = []
    for x in range(x0, pane[2] + 1):
        col = a[y0:y1, x]
        n = int((np.abs(col - np.array(colour)).max(axis=1) <= 8).sum())
        if n >= 30 * s:
            out.append(x)
    return out


def step_scroll(win, s, R):
    q2.run(win, 'seq 1 400', f'setup S1 at scale {s}', timeout=20)
    pane = R['pane']
    cx, cy = (pane[0] + pane[2]) // 2, (pane[1] + pane[3]) // 2
    win.x('mousemove', '--window', win.wid, str(cx), str(cy))
    time.sleep(.5)
    if FAULT != 'noscroll':
        for _ in range(3):
            win.x('click', '4')
            time.sleep(.08)
    t = time.time()
    win.x('mousemove', '--window', win.wid, str(int(pane[0] + 40 * s)), str(cy))
    a = A(win.shot('scrolled'))
    fg, bg = q2.rgb(THEME['colors']['foreground']), R['pane_bg']
    want_c = tuple(b + .15 * (f - b) for b, f in zip(bg, fg))
    cols = bar_columns(a, pane, want_c, s, pane[1], pane[3])
    if want('S1') and not cols:
        check('S1', s, False, f'an overlay scrollbar of {tuple(round(v) for v in want_c)} near the pane\'s '
                              f'right edge within 0.5 s of scrolling [P]')
    elif want('S1'):
        width = cols[-1] - cols[0] + 1
        gap = pane[2] - cols[-1]
        check('S1', s, len(cols) == width and abs(width - 8 * s) <= 1 and abs(gap - 7 * s) <= 1,
              f'scrollbar {width} px wide (columns {cols[0]}..{cols[-1]}), {gap} px from the pane edge {pane[2]}, '
              f'expected {8 * s:g} and {7 * s:g} +-1 [P]')
        mid = (cols[0] + cols[-1]) // 2
        ys = np.where(np.abs(a[pane[1]:pane[3], mid] - np.array(want_c)).max(axis=1) <= 8)[0] + pane[1]
        yt, ym = ys[0], (ys[0] + ys[-1]) // 2
        row = lambda y: int((np.abs(a[y, cols[0]:cols[-1] + 1] - np.array(want_c)).max(axis=1) <= 8).sum())
        check('S1', s, row(yt) < row(ym), f'rounded ends: top row {row(yt)} px of bar colour, middle row {row(ym)} [P]')
    if want('S2'):
        time.sleep(max(0, t + 3 - time.time()))
        a3 = A(win.shot('faded'))
        gone = bar_columns(a3, pane, want_c, s, pane[1], pane[3])
        check('S2', s, not gone, f'scrollbar gone 3 s after scrolling (columns {gone[:3]}) [P]')
        time.sleep(max(0, t + 6.2 - time.time()))
        late = q1.frames_between(win, t + 3.5, t + 6)
        check('S2', s, not late, f'0 frames from 3.5 s to 6 s after scrolling (observed {len(late)}) [R1]')


def step_menu(win, s, R):
    pane = R.get('pane')
    h, w = win.ph, win.pw
    x, y = (int((pane[0] + pane[2]) / 2), int(pane[1] + (pane[3] - pane[1]) * .3)) if pane else (int(w * .55), int(h * .3))
    win.click(x, y, 3)
    time.sleep(.8)
    a0 = A(win.shot('menu'))
    win.key('Down')
    time.sleep(.5)
    a1 = A(win.shot('menu-1'))
    win.key('Down')
    time.sleep(.5)
    a2 = A(win.shot('menu-2'))
    win.key('Escape')
    time.sleep(.5)
    from scipy import ndimage
    d1 = diff_mask(a0, a1)
    lab, n = ndimage.label(d1)
    require(n, f'M1 at scale {s}: Down highlights a menu row')
    sizes = ndimage.sum(d1, lab, range(1, n + 1))
    k = int(np.argmax(sizes)) + 1
    ys, xs = np.where(lab == k)
    box1 = (xs.min(), ys.min(), xs.max(), ys.max())
    m1 = lab == k
    d2 = diff_mask(a0, a2)
    lab2, n2 = ndimage.label(d2)
    require(n2, f'M1 at scale {s}: a second Down highlights the next row')
    k2 = int(np.argmax(ndimage.sum(d2, lab2, range(1, n2 + 1)))) + 1
    pitch = int(np.where(lab2 == k2)[0].min()) - box1[1]
    hh = box1[3] - box1[1] + 1
    keys, counts = np.unique(a1[m1].reshape(-1, 3), axis=0, return_counts=True)
    r = radius(coverage(a1, a0, tuple(int(v) for v in keys[counts.argmax()])), box1, 6 * s)
    check('M1', s, abs(pitch - 32 * s) <= 1, f'menu row pitch {pitch} px, expected {32 * s:g} +-1 [P]')
    check('M1', s, 26 * s - 1 <= hh <= 32 * s + 1, f'menu highlight {hh} px tall, expected {26 * s:g}..{32 * s:g} [P]')
    check('M1', s, abs(r - 6 * s) <= 2.5, f'menu highlight radius {r:g}, expected {6 * s:g} +-2.5 [P]')


def step_focus(win, s, R):
    win.key('ctrl+comma')
    time.sleep(1.5)
    win.away()
    a0 = A(win.shot('settings'))
    win.key('Tab')
    time.sleep(.6)
    a1 = A(win.shot('settings-tab'))
    win.key('Escape')
    time.sleep(.5)
    from scipy import ndimage
    d = diff_mask(a0, a1, 12)
    lab, n = ndimage.label(d)
    require(n, f'K1 at scale {s}: Tab in Settings changes the window (a focus ring) [P]')
    best = None
    for i in range(1, n + 1):
        ys, xs = np.where(lab == i)
        area = (xs.max() - xs.min() + 1) * (ys.max() - ys.min() + 1)
        if best is None or area > best[0]:
            best = (area, (xs.min(), ys.min(), xs.max(), ys.max()), lab == i)
    _, box, m = best
    bw, bh = box[2] - box[0] + 1, box[3] - box[1] + 1
    ym, xm = (box[1] + box[3]) // 2, (box[0] + box[2]) // 2
    left = 0
    while box[0] + left <= box[2] and m[ym, box[0] + left]:
        left += 1
    top = 0
    while box[1] + top <= box[3] and m[box[1] + top, xm]:
        top += 1
    check('K1', s, bw >= 16 * s and bh >= 16 * s and abs(left - 2 * s) <= 1 and abs(top - 2 * s) <= 1,
          f'focus ring box {bw} x {bh}, band {left} px at the left edge and {top} px at the top, expected '
          f'{2 * s:g} +-1 [P]')


def step_split(win, s, R):
    n = len(win.terminals())
    win.key('ctrl+shift+p')
    time.sleep(.8)
    win.type('Split right')
    win.key('Return')
    until(lambda: len(win.terminals()) == n + 1, f'setup G1 at scale {s}: "Split right" adds a pane', 10)
    time.sleep(2)
    win.away()
    a = A(win.shot('split'))
    h, w = a.shape[:2]
    pane = R.get('pane') or (int(244 * s), 0, w - 1, h - 1)
    y = int(h * .5)
    mid = (pane[0] + pane[2]) // 2
    lc = tuple(int(v) for v in a[y, int(mid - 60 * s)])
    rc = tuple(int(v) for v in a[y, int(mid + 60 * s)])
    x = int(mid - 60 * s)
    while x < w - 1 and cdiff(a[y, x + 1], lc) <= 2:
        x += 1
    left_end = x
    x = int(mid + 60 * s)
    while x > 0 and cdiff(a[y, x - 1], rc) <= 2:
        x -= 1
    gap = x - left_end - 1
    check('G1', s, abs(gap - 8 * s) <= 1, f'gap between split panes {gap} px (x {left_end + 1}..{x - 1}), expected '
                                          f'{8 * s:g} +-1 [R1]')


STEPS = [('top', step_top), ('idle', step_idle), ('footer', step_footer), ('input', step_input),
         ('search', step_search), ('tabs', step_tabs), ('buttons', step_buttons), ('grid', step_grid),
         ('sharp', step_sharp), ('scroll', step_scroll), ('menu', step_menu), ('focus', step_focus),
         ('split', step_split)]


def scale_story(s):
    need = steps_needed()
    if want('X1'):
        need |= {'grid', 'footer', 'sharp'}
    if FAULT == 'line' or want('L1'):
        need |= {'top'}
    cfg = dict(CFG, font_size=13) if FAULT == 'font' else dict(CFG)
    env_scale = 1 if (FAULT == 'scale' and s == 2) else None
    win = Win(f's{s}', cfg, s, env_scale)
    R = RESULTS.setdefault(s, {})
    try:
        for name, fn in STEPS:
            if name in need:
                fn(win, s, R)
    finally:
        win.close()


# ---------------------------------------------------------------- live run on DP-2 (native Wayland)

def live():
    """D1, C1 and P1 on Marc's DP-2 (scale 1.25), a floating 1200 x 800 window, captured with grim."""
    require(os.environ.get('WAYLAND_DISPLAY') and shutil.which('hyprctl') and shutil.which('grim'),
            'live: a Wayland session with hyprctl and grim')
    mons = json.loads(subprocess.run(['hyprctl', 'monitors', '-j'], capture_output=True, text=True).stdout)
    dp2 = next((m for m in mons if m['name'] == 'DP-2'), None)
    require(dp2, 'live: monitor DP-2 present')
    s = float(dp2['scale'])
    require(abs(s - 1.25) < .01, f'live: DP-2 runs at scale 1.25 (observed {s})')
    w = ROOT / 'live'
    w.mkdir(parents=True, mode=0o700)
    for d in ('home', 'config', 'data', 'state'):
        (w / d).mkdir(mode=0o700)
    home = w / 'home'
    log = w / 'c3prog.log'
    (home / 'c3prog').write_text(PROG)
    (home / 'c3prog').chmod(0o755)
    shell = w / 'shell.sh'
    shell.write_text(f'#!/bin/sh\nexec {home / "c3prog"} {log} "{TERM_TEXT}"\n')
    shell.chmod(0o755)
    cfgp = w / 'config.json'
    cfgp.write_text(json.dumps(CFG))
    sock, pidf = w / 'window.sock', w / 'window.pid'
    launch = w / 'launch.sh'
    env = {k: v for k, v in os.environ.items() if not k.startswith(('CLANKY', 'XDG_CONFIG', 'XDG_DATA', 'XDG_STATE'))}
    keep = ' '.join(f'{k}={json.dumps(v)}' for k, v in sorted(env.items())
                    if k in ('WAYLAND_DISPLAY', 'XDG_RUNTIME_DIR', 'PATH', 'LANG', 'HYPRLAND_INSTANCE_SIGNATURE'))
    launch.write_text(f'#!/bin/sh\necho $$ > {pidf}\nexec env -i {keep} HOME={home} XDG_CONFIG_HOME={w / "config"} '
                      f'XDG_DATA_HOME={w / "data"} XDG_STATE_HOME={w / "state"} SHELL={shell} TZ=UTC '
                      f'WINIT_UNIX_BACKEND=wayland CLANKY_FRAME_TRACE={w / "frames.csv"} '
                      f'{q2.APP} --socket {sock} --config {cfgp}\n')
    launch.chmod(0o755)
    subprocess.run(['hyprctl', 'dispatch', 'exec', f'[float;monitor DP-2;size 1200 800;center] {launch}'],
                   check=True, stdout=subprocess.DEVNULL)
    pid = None
    server = None
    try:
        pid = int(until(lambda: pidf.exists() and pidf.read_text().strip(), 'live: the window process starts', 20))
        cl = until(lambda: next((c for c in json.loads(subprocess.run(['hyprctl', 'clients', '-j'], capture_output=True,
                                                                       text=True).stdout) if c.get('pid') == pid), None),
                   'live: the window maps', 40)
        conn = t2.Conn(sock)
        conn.call(3, 'auth', token=Path(str(sock) + '.owner').read_text().strip())
        server = conn.call(3, 'hello').get('server_pid')
        size = Path(str(log) + '.size')
        until(lambda: size.exists() and size.read_text().strip(), 'live: the program runs', 30)
        time.sleep(2)
        cl = next(c for c in json.loads(subprocess.run(['hyprctl', 'clients', '-j'], capture_output=True,
                                                       text=True).stdout) if c.get('pid') == pid)
        (x, y), (cw_, ch_) = cl['at'], cl['size']
        shot = w / 'live.png'
        subprocess.run(['grim', '-g', f'{x},{y} {cw_}x{ch_}', str(shot)], check=True)
        a = A(Image.open(shot).convert('RGB'))
        R = {}
        bar, hgt = title_bar(a, s)
        R['bar_h'] = hgt
        check('D1', s, abs(hgt - 46 * s) <= 1, f'live DP-2: title bar {hgt} px, expected {46 * s:g} +-1 [R1, P]')
        mask = np.abs(a - np.array(MAG)).max(axis=2) <= 2
        from scipy import ndimage
        lab, n = ndimage.label(mask)
        boxes = sorted([(np.where(lab == i)[1].min(), np.where(lab == i)[0].min()) for i in range(1, n + 1)
                        if (lab == i).sum() > 20], key=lambda b: (b[1], b[0]))
        require(len(boxes) == 2, f'live: two magenta cells ({boxes})')
        (ax, ay), (bx, by) = boxes
        cw, ch = (bx - ax) / 20, (by - ay) / 10
        check('C1', s, abs(cw - CELL_W * s) <= 1 and abs(ch - CELL_H * s) <= 1,
              f'live DP-2: cell {cw:g} x {ch:g}, expected {CELL_W * s:g} x {CELL_H * s:g} +-1 [R1, F]')
        pane_bg = tuple(int(v) for v in a[int(ay + 8.5 * ch), int(ax + 12.5 * cw)])
        xx = ax - 2
        while xx > 0 and cdiff(a[int(ay + ch / 2), xx], pane_bg) <= 2:
            xx -= 1
        yy = ay - 2
        while yy > 0 and cdiff(a[yy, int(ax + 1.5 * cw)], pane_bg) <= 2:
            yy -= 1
        run_top = yy + 1
        pane_top = hgt if 0 <= run_top - hgt <= math.ceil(4 * s) else run_top
        lp, tp = ax - (xx + 1), ay - pane_top
        check('P1', s, abs(lp - 14 * s) <= 1 and abs(tp - 14 * s) <= 1,
              f'live DP-2: padding left {lp}, top {tp}, expected {14 * s:g} +-1 [R1]')
        print('PASS: C3 live DP-2')
    finally:
        for p in (server, pid):
            if p:
                try:
                    os.kill(p, 15)
                except ProcessLookupError:
                    pass


def main():
    for tool in ('Xvfb', 'xdotool', 'xclip', 'ffmpeg', 'tesseract'):
        require(shutil.which(tool), f'tool {tool} installed')
    plex = subprocess.run(['fc-match', '-f', '%{file}', 'IBM Plex Mono:style=Regular'], capture_output=True,
                          text=True).stdout
    if 'Plex' in plex:
        require(natural_height(plex, TERM_PX) == 21, 'setup: the natural-height rule gives Ptyxis\'s 21 px for IBM Plex Mono 11')
    print(f'expected: terminal cell {CELL_W:g} x {CELL_H} px at scale 1 (Geist Mono natural height {NAT} px)')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    if LIVE:
        live()
        shutil.rmtree(ROOT, ignore_errors=True)
        return
    scales = FAULT_SCALES[FAULT] if FAULT else SCALES
    if '--scales' in ARGS:
        scales = tuple(float(v) if '.' in v else int(v) for v in ARGS[ARGS.index('--scales') + 1].split(','))
    if ONLY is not None and not FAULT and ONLY & {'X1', 'L1'}:
        scales = tuple(sorted(set(scales) | {1}))
    for s in scales:
        scale_story(s)
    require(not FAILED, FAILED[0] if FAILED else '')
    if FAULT is None:
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: C3 sizes' if ONLY is None else f'PASS: C3 checks {",".join(sorted(ONLY))}')


if __name__ == '__main__':
    known = (None,) + tuple(FAULT_CHECK)
    try:
        require(FAULT in known, f'known fault {FAULT}')
        main()
    except Fail as e:
        print(f'FAIL: C3 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: C3 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
