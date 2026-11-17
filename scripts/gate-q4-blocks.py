#!/usr/bin/env python3
"""Q4 gate: breathing blocks (design 2026-11-14, milestone Q4; mockup Marc approved:
/srv/devops/build/clanky-redesign/mockup.html section 1, CSS .blocks .blk .blk.fail .meta .cmd .sticky at lines 71-86,
markup at lines 215-232 and 311).

Usage: gate-q4-blocks.py [--fault fail|select|alt|naive|idle|classic|osc]
A fault changes the run's input on purpose; a correct build must then FAIL.

Harness: the Q2 gate's window (scripts/gate-q2-input.py Win, itself the Q1 and T2 harness: free_display from :100 up),
a real Clanky window on a private Xvfb display, 1400 x 900 px at scale 1, software Vulkan, IBM Plex Mono 11 pt at
cell_height_scale 1.25 (9 x 26 px cells [P]), cursor blink off, theme Graphite dark, TZ=UTC, the tools panel on,
command_input "editor" and block_style "quiet" (the defaults). The shell is real bash (SHELL=/bin/bash) with Clanky's
own OSC 133 integration; the gate's ~/.bashrc sources /etc/bashrc, puts ~/bin first on PATH, sets PS1='$ ' and
cd's to ~/alpha. Screenshots leave out the pointer. Commands are typed into the command input with xdotool and run
with Enter, one block each (the Q2 gate's run(), which waits for the block on the owner API's list_blocks).
Pixel rows are measured, never assumed: the gate's ~/bin/mark prints one output row that starts with a 24-bit
background cell (ESC[48;2;R;G;Bm), so each block's output row has a cell whose exact top and bottom pixel rows are
found in the screenshot; the row above an output row is its command row (bash echoes the typed line, then the output
follows on the next row [B]).
  mark COLOUR EXIT SLEEP WORD...   sleeps SLEEP s, prints  <cell><space>WORD... , exits EXIT.
  Colours: m (255,0,255), c (0,255,255), y (255,255,0), g (0,255,0).
The three blocks of the main window, on a fresh screen:
  K1 "mark m 0 0 one" (exit 0), K2 "mark c 3 0 two" (exit 3, failed), K3 "mark y 0 2.3 kiwi lemon mango"
  (exit 0, about 2.3 s). Screen rows 1-6 (owner API) are the three command rows and three output rows.

Interface fixed by this gate: no new config keys. The gaps are drawing only: the grid, the screen the program sees
and the owner API's rows do not change.

Sources: [M] the mockup CSS: .blk padding 9px 18px 11px, border-top 1px, .blk:first-child border-top 0,
.blk.fail border-top-color rgba(236,138,124,.45), .meta margin-bottom 2px, .meta .right margin-left auto (duration
and exit on the right, exit before the duration: markup line 231), folder (.cwd) then HH:MM on the left;
.sticky top 0, padding 4px, command bold, "running N s" right. [B] the design, round 2: 9 px above the meta row, 11 px
below the last output row, the hairline in that gap and none above the first block, the meta row its own row above
the bold command, 12 px meta text (the window's text floor wins over the mockup's 10.5 px), no gaps on the
alternate screen, in classic command_input or without OSC 133 ("the plain grid exactly as today"), the mouse maps
through the gaps, scrolling keeps the gaps, the sticky line still works, 0 idle frames. Bash semantics for echo and
$?. [P] the T2 gate's Ptyxis/VTE cell measurement, 9 x 26 px. [T] assets/themes/graphite-dark.json: background
#111111 (17,17,17), border #ffffff1a over it (41,41,41) (the mockup's --g-line .08 over #111111 is (36,36,36):
the tolerance 5 takes both), err #ec8a7c (236,138,124). [X] xterm ctlseqs: SGR mouse (1000 + 1006) press
ESC[<0;col;rowM, release ESC[<0;col;rowm, 1-based cells of the program's screen; DSR 6 answers ESC[row;colR.
[V] VTE / Ptyxis selection: a drag selects every cell from the press cell to the release cell, rows joined by "\\n",
trailing blanks of each row dropped; Shift with no mouse mode is a plain selection; double-click selects a word.
[Q1] the Q1 gate G1: today's grid origin y is 42..70 (title bar 38 px [M], no pane header).
The failed hairline rgba(236,138,124,.45) over #111111 is (116,72,65) [M, T].

Arithmetic fixed before the work. bottom(Kn) = the pixel row under Kn's output cell (its top + 26); cmd(Kn) = Kn's
output cell top - 26. Between two blocks: 11 px [M padding-bottom] + 1 px hairline [M border-top] + 9 px
[M padding-top] + the meta row + 2 px [M margin-bottom]. The meta row holds 12 px text [B]: its height is 16-23 px
(line box 1.33-1.92 em; the mockup's own .blocks line-height 1.55 gives 18.6 px, the cell's 26 / 14.7 px = 1.77
gives 21.3 px). So cmd(Kn+1) - bottom(Kn) is 11 + 1 + 9 + 16..23 + 2 = 39..46 px.

Expected results:
  G1 no hairline above the first block: no pixel row from y 39 to cmd(K1) has >= 90 % of x 300..1300 (step 3)
     within 5 of (41,41,41) or within 8 of (116,72,65). K1's meta text starts at least 9 px below the title bar:
     the first row with ink (a pixel more than 40 from the background, x from the cell column to W - 4) above
     cmd(K1) is at y >= 38 + 9 = 47.
  G2 for K1 -> K2 (hairline (116,72,65) +-8, K2 failed) and K2 -> K3 (hairline (41,41,41) +-5):
     a. cmd(Kn+1) - bottom(Kn) in 39..46 (the arithmetic above).
     b. exactly one hairline of 1-2 px between bottom(Kn) - 3 and cmd(Kn+1) + 3, its first row y_h with
        y_h - bottom(Kn) in 10..12 (11 px below the last output row [B, M]): inside the gap, not on a cell edge.
     c. the space is empty: every row from bottom(Kn) to y_h - 2 and from y_h + 2 to y_h + 9 (the 9 px above the meta
        row) has >= 98 % of x 300..1300 within 4 of (17,17,17).
     d. the meta row has ink between y_h + 10 and cmd(Kn+1) - 1, and the command row has ink in its first 4 cells.
  M1 the meta row of each block (OCR of the band from y_h + 2, or y 39 for K1, to cmd(Kn), read seven ways because
     faint small text misreads: the Q2 gate's contrast-stretched faint_words at scales 3, 4, 5 and plain OCR at
     scale/psm 4/11, 4/6, 6/6, 3/11; the row passes when one reading has every part; the parts do not change):
     LEFT: the leftmost word starts within -3..+12 px of the cell column's left edge (the grid's first column,
     .blk padding-left 18 [M]), a word with "alpha" (the folder ~/alpha) starts at most 30 px right of it, and a word
     HH:MM equal to the block's start minute in UTC (or the minute 5 s later) lies right of the folder and left of
     the pane's middle (x 818 = (236 + 1400) / 2). RIGHT: the rightmost word ends at x 1372..1385 (pane right edge
     1400 - 18 [M], +3/-10). K2 reads "exit 3" in the right half, before its duration; K1 and K3 have no "exit";
     K3's duration reads N.N s with N.N in 2.0..3.5.
  M2 err colour (within 40 of (236,138,124), red above green by 40): >= 8 pixels in K2's meta band right of x 818,
     none in K1's or K3's meta band, none on K2's command row (the old place of "exit 3").
  I1 0 frames from 1 s to 3.5 s after the blocks settle (the 0-idle-frames rule).
  S1 selection through a gap: no mouse mode, a Shift+drag from 2 px into column 3 of K2's output row (the "t" of
     "two") to 7 px into column 12 of K3's output row (the "n" of "lemon"), pointer y at the measured cell middles,
     makes PRIMARY exactly "two\\nmark y 0 2.3 kiwi lemon mango\\n  kiwi lemon" [V, B]: the meta row and the gaps add
     nothing. S2 a double-click in column 10 of K3's output row makes PRIMARY "lemon".
  A1 alternate screen: the mouse gate's PROG with ESC[?1049h paints magenta cells at (1,2), (1,9), (1,20) over the
     blocks: exactly three magenta cells, tops 182 and 468 px below the first (7 and 18 rows of 26 px, +-1), the
     grid origin (first top - 26) in 42..70 [Q1], and no hairline row (G1's rule) from y 39 to the last cell.
  R1 mouse reporting below a gap: ~/bin/mrep turns on 1000 + 1006, asks DSR 6 for its row R, paints a green cell at
     column 1 of row R and logs what it reads. A click 4 px into column 5 at the green cell's middle sends exactly
     ESC[<0;5;RM ESC[<0;5;Rm; a click 4 px into column 10 at the middle of K3's output cell row sends exactly
     ESC[<0;10;R3M ESC[<0;10;R3m, R3 the 1-based screen row of "kiwi lemon mango" on the owner API [X].
     Setup: the owner API's row of "target" is R.
  V1 the newest row stays in view: after "seq 1 80; mark g 0 0 lastrow" its green cell lies wholly between y 39 and
     the command input's hairline (the Q2 gate's hairline(), else y 874).
  W1 wheel scroll keeps the gaps: wheel up one notch at a time (at most 80) until K2's cyan cell top is >= 90 (below
     a sticky line) and K3's yellow cell bottom is above the input; there G2 a-c hold for K2 -> K3 and the gap equals
     the one measured at rest (+-1). W2 0 frames from 1 s to 3.5 s after the last notch. Then wheel down (at most 80
     notches) brings lastrow's green cell back into view.
  T1 sticky command line: ": stickyjob; seq 1 150; sleep 12" (its command and meta rows scroll off): the strip
     y 38..84 at the pane top reads "stickyjob" and "running N s"; 3 s later N is 2-5 higher; 2-8 frames in those
     4 s; after the block ends "running" is gone and 0 idle frames (T2).
  C1 classic: a window with command_input "classic" (bash, PS1 '$ '), K1..K3 typed at bash's own prompt: the three
     output cells' tops are exactly 52 and 104 px below the first (+-1, rows 2, 4, 6 of 26 px), the grid origin
     (first top - 26) in 42..70; and the three OSC 133 blocks exist (list_blocks, K2 exit 3).
  N1 without OSC 133: a window whose shell is the Q2 gate's minish (no OSC 133), editor mode: the same three
     commands give output cells 52 and 104 px apart (+-1), origin in 42..70; list_blocks has no block.
On clanky-gpui (CLANKY_BIN, G2): the same checks and arithmetic on its cell (8.8 x 18 px, from the font file) and its
colours: background #141414, dividers foreground 8 % (38,38,38) and err 50 % (126,73,73) over it, err #e97e7e (the
Graphite dark theme file and the rulebook's "block" tokens). Its meta text is the rulebook's text_meta, 10 px (Marc
kept 10 px, 2026-11-16), in an 18 px meta row; its space between blocks is 12 + 1 + 8 + 18 + 2 = 41 px, inside 39..46.
Nothing here checks a shadow under the sticky line; clanky-gpui draws none (the rulebook gives shadows to floating
things only).
Planted faults: fail (K2 exits 0, so no red hairline), select (the drag ends in column 6, "kiwi"), alt (the
alternate-screen program is not started, so the blocks' own cells are measured), naive (R1 clicks at the y a grid
without gaps would have: cmd(K1) + (row - 1) x 26 + 13), idle (a running "sleep 20" during I1), classic (C1's window
is started with command_input "editor"), osc (N1's window runs bash with Clanky's OSC 133 instead of minish).
"""
import importlib.util, json, os, re, shutil, subprocess, sys, time
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_Q4_WORK', '/srv/devops/build/clanky-q4-gate-run'))
os.environ['CLANKY_Q2_WORK'] = str(ROOT)
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
_argv = sys.argv
sys.argv = sys.argv[:1]  # the harness modules read their own --fault from argv; they get none
spec = importlib.util.spec_from_file_location('q2', Path(__file__).with_name('gate-q2-input.py'))
q2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q2)
sys.argv = _argv
q1, gm, t2 = q2.q1, q2.gm, q2.t2
Fail, require, until, near = q1.Fail, q1.require, q1.until, q1.near
W, H = 1400, 900
BG, LINE_BG, FAIL_LINE, ERR = (17, 17, 17), (41, 41, 41), (116, 72, 65), (236, 138, 124)
MAG, CYAN, YEL, GRN = (255, 0, 255), (0, 255, 255), (255, 255, 0), (0, 255, 0)
CW, CH = 9, 26
# The last row above the pane's text area: today's window ends its 38 px title bar there [M].
TOP = 38
GPUI = t2.gate_app.override() is not None
if GPUI:
    # clanky-gpui's cell for the same config (IBM Plex Mono 11 pt, cell_height_scale 1.25), from the font file:
    # 8.8 x 18 px (its line height is the font size x the scale; today's window scales the font's natural height).
    CW, CH = t2.gate_app.gpui_cell('IBM Plex Mono', 11, 1.25)
    # Its colours: the Graphite dark theme it draws with (the terminal takes the theme's background) and the
    # rulebook's dividers, foreground at 8 % and err at 50 % over the background (rulebook-tokens.json "block").
    _theme = json.loads((q2.REPO / 'assets/themes/graphite-dark.json').read_text())['colors']
    _block = json.loads((q2.REPO / 'scripts/window-chrome/rulebook-tokens.json').read_text())['block']
    _rgb = lambda h: tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))
    _over = lambda a, b, t: tuple(round(x + (y - x) * t) for x, y in zip(a, b))
    BG, ERR = _rgb(_theme['background']), _rgb(_theme['err'])
    LINE_BG = _over(BG, _rgb(_theme['foreground']), _block['divider_foreground_alpha']['value'])
    FAIL_LINE = _over(BG, ERR, _block['divider_failed_err_alpha']['value'])
    # The pane's inside starts under the 40 px title bar, the 8 px margin and its one-pixel edge (the rulebook's
    # h_title_bar and pane margin): the rows above it are the window's frame.
    _tokens = json.loads((q2.REPO / 'scripts/window-chrome/rulebook-tokens.json').read_text())
    TOP = _tokens['heights']['h_title_bar']['px'] + _tokens['pane']['margin']['px'] + 1
MID = (236 + W) // 2
E = '\x1b'
BASE = dict(q1.BASE, theme='dark', theme_dark='graphite-dark')
BASHRC = '[ -f /etc/bashrc ] && . /etc/bashrc\nPATH="$HOME/bin:$PATH"\nPS1=\'$ \'\ncd ~/alpha\n'
MARK = r'''#!/bin/sh
# mark COLOUR EXIT SLEEP WORD...: one output row, a 24-bit background cell, a space, the words
c=$1; e=$2; s=$3; shift 3
case $c in m) rgb='255;0;255';; c) rgb='0;255;255';; y) rgb='255;255;0';; g) rgb='0;255;0';; esac
[ "$s" = 0 ] || sleep "$s"
printf '\033[48;2;%sm \033[0m %s\n' "$rgb" "$*"
exit "$e"
'''
MREP = r'''#!/usr/bin/python3 -I
import json, os, select, sys, termios, tty
log = sys.argv[1]
fd = 0
old = termios.tcgetattr(fd)
tty.setraw(fd)
w = lambda s: os.write(1, s.encode())
try:
    w("\x1b[?1000h\x1b[?1006h\x1b[6n")
    buf = b""
    while not buf.endswith(b"R"):
        buf += os.read(fd, 1)
    row = buf.rsplit(b"[", 1)[1].split(b";")[0].decode()
    w("\x1b[48;2;0;255;0m \x1b[0m target\r\n")
    open(log + ".row", "w").write(row)
    while True:
        r, _, _ = select.select([fd], [], [], 90)
        if not r:
            break
        data = os.read(fd, 4096)
        if data == b"q":
            break
        with open(log, "a") as f:
            f.write(json.dumps(data.decode("latin-1")) + "\n")
finally:
    w("\x1b[?1006l\x1b[?1000l")
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
'''
K1, K3 = 'mark m 0 0 one', 'mark y 0 2.3 kiwi lemon mango'


def k2(code=3):
    return f'mark c {code} 0 two'


def make_home(home):
    home.mkdir(mode=0o700, exist_ok=True)
    (home / '.bashrc').write_text(BASHRC)
    (home / 'alpha').mkdir(exist_ok=True)
    b = home / 'bin'
    b.mkdir(exist_ok=True)
    for name, text in (('mark', MARK), ('mrep', MREP), ('minish', q2.MINISH)):
        (b / name).write_text(text)
        (b / name).chmod(0o755)


q2.make_home = make_home
Win = q2.Win


# ---------------------------------------------------------------- pixels

def marker_cells(im, colours=(MAG, CYAN, YEL, GRN), region=(237, TOP + 1, W, 880)):
    """Boxes (x0, y0, x1, y1) of exact-colour cells, per colour."""
    import numpy as np
    a = np.asarray(im.crop(region)).astype(int)
    out = {}
    for col in colours:
        mask = (np.abs(a - np.array(col)).max(axis=2) <= 2)
        ys = np.where(mask.any(axis=1))[0]
        boxes = []
        groups = []
        for y in ys:
            if groups and y - groups[-1][-1] <= 1:
                groups[-1].append(y)
            else:
                groups.append([y])
        for g in groups:
            xs = np.where(mask[g[0]:g[-1] + 1].any(axis=0))[0]
            runs = []
            for x in xs:
                if runs and x - runs[-1][1] <= 2:
                    runs[-1][1] = x
                else:
                    runs.append([x, x])
            for xa, xb in runs:
                boxes.append((int(xa) + region[0], int(g[0]) + region[1], int(xb) + 1 + region[0], int(g[-1]) + 1 + region[1]))
        out[col] = boxes
    return out


XS = range(300, 1300, 3)


def frac(im, y, colour, tol):
    px = im.load()
    return sum(near(px[x, y], colour, tol) for x in XS) / len(XS)


def hair_rows(im, y0, y1, colour, tol):
    return [y for y in range(max(0, y0), min(H, y1)) if frac(im, y, colour, tol) >= .9]


def any_hair(im, y0, y1):
    return hair_rows(im, y0, y1, LINE_BG, 5) + hair_rows(im, y0, y1, FAIL_LINE, 8)


def ink_rows(im, y0, y1, x0, x1):
    import numpy as np
    a = np.asarray(im.crop((x0, y0, x1, y1))).astype(int)
    hit = (np.abs(a - np.array(BG)).max(axis=2) > 40).any(axis=1)
    return [y0 + int(i) for i in np.where(hit)[0]]


def err_count(im, box):
    import numpy as np
    a = np.asarray(im.crop(box)).astype(int)
    m = (np.abs(a - np.array(ERR)).max(axis=2) <= 40) & (a[:, :, 0] > a[:, :, 1] + 40)
    return int(m.sum())


def one_cell(cells, col, label):
    got = cells[col]
    require(len(got) == 1, f'{label}: one {col} cell on screen (observed {got})')
    b = got[0]
    require(abs(b[2] - b[0] - CW) <= 1 and abs(b[3] - b[1] - CH) <= 1, f'{label}: the cell is {CW:g} x {CH} px (observed {b})')
    return b


def gap(im, prev, nxt, colour, tol, label):
    """G2 a-d between the block whose output cell is prev and the next one whose output cell is nxt; returns
    (y_h, total gap)."""
    bottom, cmd = prev[3], nxt[1] - CH
    total = cmd - bottom
    require(39 <= total <= 46, f'{label}: 39-46 px from the last output row to the next command row, 11 + 1 + 9 + '
                               f'meta row 16-23 + 2 (observed {total} px: output row ends at y {bottom}, command row '
                               f'starts at y {cmd})')
    hits = hair_rows(im, bottom - 3, cmd + 3, colour, tol)
    require(1 <= len(hits) <= 2 and hits[-1] - hits[0] == len(hits) - 1,
            f'{label}: one 1-2 px hairline {colour} in the gap (rows matching: {hits})')
    others = [y for y in any_hair(im, bottom - 3, cmd + 3) if y not in hits]
    require(not others, f'{label}: no other hairline in the gap (rows {others})')
    yh = hits[0]
    require(10 <= yh - bottom <= 12, f'{label}: the hairline sits 11 px below the last output row (observed '
                                     f'{yh - bottom} px)')
    busy = [y for y in list(range(bottom, yh - 1)) + list(range(yh + 2, yh + 10)) if frac(im, y, BG, 4) < .98]
    require(not busy, f'{label}: the 11 px below the output and the 9 px above the meta row are empty (rows with '
                      f'something on them: {busy})')
    require(ink_rows(im, yh + 10, cmd, nxt[0] - 4, W - 4), f'{label}: the meta row between y {yh + 10} and {cmd} has text')
    require(ink_rows(im, cmd + 2, cmd + CH - 2, nxt[0], round(nxt[0] + 4 * CW)), f'{label}: the command row has text')
    return yh, total


# ---------------------------------------------------------------- reading the meta row

def readings(im, box):
    out = []
    for sc in (3, 4, 5):
        out.append([(t, box[0] + a, box[1] + b, box[0] + c, box[1] + d)
                    for t, a, b, c, d in q2.faint_words(im, box, sc)])
    for sc, psm in ((4, 11), (4, 6), (6, 6), (3, 11)):
        out.append(q1.ocr_words(im, box, sc, psm))
    return out


def meta_fails(words, x1col, stamps, exit_code, dur):
    """The parts of the meta row one OCR reading gets wrong (an empty set: all right)."""
    words = sorted(words, key=lambda w: w[1])
    if not words:
        return {'empty'}
    bad = set()
    if not (x1col - 3 <= words[0][1] <= x1col + 12):
        bad.add('left edge')
    folder = next((w for w in words if 'alpha' in w[0]), None)
    if not folder or folder[1] > x1col + 30:
        bad.add('folder')
    if not any(any(s in w[0] for s in stamps) and (not folder or w[1] > folder[1]) and w[3] < MID for w in words):
        bad.add('time')
    if not (W - 18 - 10 <= max(w[3] for w in words) <= W - 18 + 3):
        bad.add('right edge')
    right = ' '.join(w[0] for w in words if w[1] > MID)
    if exit_code is None:
        if 'exit' in ' '.join(w[0] for w in words):
            bad.add('exit')
    elif not re.search(rf'exit ?{exit_code}\b.*\d', right):
        bad.add('exit')
    if dur:
        vals = [float(v) for v in re.findall(r'(\d+\.\d) ?s\b', right)]
        if not any(dur[0] <= v <= dur[1] for v in vals):
            bad.add('duration')
    return bad


def meta_ok(readings_, x1col, stamps, exit_code, dur):
    """Faint 13 px text: each part must be read right by at least one of the OCR passes ("exit" absent: by all)."""
    fails = [meta_fails(r, x1col, stamps, exit_code, dur) for r in readings_]
    if exit_code is None and any('exit' in f for f in fails):
        return False
    return bool(fails) and not set.intersection(*fails)


def meta_check(im, box, x1col, stamps, exit_code, dur, label):
    reads = readings(im, box)
    require(meta_ok(reads, x1col, stamps, exit_code, dur),
            f'M1 {label} meta row: folder alpha and {"/".join(sorted(stamps))} on the left, '
            f'{"exit " + str(exit_code) + " then " if exit_code else ""}duration'
            f'{" %.1f-%.1f s" % dur if dur else ""} on the right edge (x 1372-1385) '
            f'(read {[" ".join(f"{w[0]}@{int(w[1])}-{int(w[3])}" for w in sorted(r, key=lambda w: w[1])) for r in reads]!r})')


# ---------------------------------------------------------------- main window

def setup_blocks(win, fail_code):
    start = time.time()
    for text, code, label in ((K1, 0, 'K1'), (k2(fail_code), fail_code, 'K2'), (K3, 0, 'K3')):
        b = q2.run(win, text, f'setup {label}', timeout=12)
        require(q2.cmd(b) == text and b.get('exit_code') == code,
                f'setup {label}: runs {text!r} with exit {code} (observed {q2.cmd(b)!r}, {b.get("exit_code")})')
    want = [K1, 'one', k2(fail_code), 'two', K3, 'kiwi lemon mango']
    rows = [r.strip() for r in q2.screen_rows(win)]
    require(rows[:6] == want, f'setup: screen rows 1-6 are {want} (observed {rows[:8]})')
    win.away()
    time.sleep(1.2)
    return start


def geometry(win, start):
    im = win.shot('blocks')
    cells = marker_cells(im)
    m, c, y = (one_cell(cells, col, f'setup {k}') for col, k in ((MAG, 'K1'), (CYAN, 'K2'), (YEL, 'K3')))
    require(abs(m[0] - c[0]) <= 1 and abs(m[0] - y[0]) <= 1, f'setup: the three cells share column 1 ({m}, {c}, {y})')
    x1 = m[0]
    cmd1 = m[1] - CH
    hits = any_hair(im, TOP + 1, cmd1)
    require(not hits, f'G1 no hairline above the first block (rows {hits}, first command row at y {cmd1})')
    ink = ink_rows(im, TOP, cmd1, x1 - 4, W - 4)
    require(ink and ink[0] >= TOP + 9, f'G1 the first block\'s meta text starts >= 9 px below the title bar (first ink '
                                  f'row {ink[:1]}, command row at y {cmd1})')
    h2, _ = gap(im, m, c, FAIL_LINE, 8, 'G2 K1 -> failed K2')
    h3, g3 = gap(im, c, y, LINE_BG, 5, 'G2 K2 -> K3')
    print('ok: G1 G2 gaps and hairlines')
    stamps = {time.strftime('%H:%M', time.gmtime(start + d)) for d in (0, 5)}
    bands = {'K1': (x1 - 6, max(TOP + 1, ink[0] - 3), W - 2, cmd1), 'K2': (x1 - 6, h2 + 2, W - 2, c[1] - CH),
             'K3': (x1 - 6, h3 + 2, W - 2, y[1] - CH)}
    meta_check(im, bands['K1'], x1, stamps, None, None, 'K1')
    meta_check(im, bands['K2'], x1, stamps, 3, None, 'K2')
    meta_check(im, bands['K3'], x1, stamps, None, (2.0, 3.5), 'K3')
    e2 = err_count(im, (MID, bands['K2'][1], W - 2, bands['K2'][3]))
    e1, e3 = err_count(im, bands['K1']), err_count(im, bands['K3'])
    e2cmd = err_count(im, (x1, c[1] - CH + 2, W - 2, c[1] - 2))
    require(e2 >= 8 and e1 == 0 and e3 == 0 and e2cmd == 0,
            f'M2 err colour only in the failed block\'s meta row (K2 meta right {e2}, K1 {e1}, K3 {e3}, '
            f'K2 command row {e2cmd})')
    print('ok: M1 M2 meta rows')
    return dict(x1=x1, m=m, c=c, y=y, cmd1=cmd1, gap3=g3)


def col_x(g, col, dx):
    return round(g['x1'] + (col - 1) * CW + dx)


def select_story(win, g):
    cy, yy = g['c'][1] + CH // 2, g['y'][1] + CH // 2
    win.click(col_x(g, 70, 4), cy)  # an empty cell: clears any selection
    end_col = 6 if FAULT == 'select' else 12
    win.x('keydown', 'shift')
    win.x('mousemove', '--window', win.wid, str(col_x(g, 3, 2)), str(cy))
    time.sleep(.15)
    win.x('mousedown', '1')
    time.sleep(.2)
    win.x('mousemove', '--window', win.wid, str(col_x(g, 8, 4)), str((cy + yy) // 2))
    time.sleep(.15)
    win.x('mousemove', '--window', win.wid, str(col_x(g, end_col, 7)), str(yy))
    time.sleep(.2)
    win.x('mouseup', '1')
    win.x('keyup', 'shift')
    time.sleep(.4)
    want = 'two\n' + K3 + '\n  kiwi lemon'
    got = gm.primary(win)
    require(got == want, f'S1 Shift+drag across two blocks selects {want!r} (PRIMARY {got!r})')
    win.x('mousemove', '--window', win.wid, str(col_x(g, 10, 4)), str(yy))
    time.sleep(.15)
    win.x('click', '--repeat', '2', '--delay', '80', '1')
    time.sleep(.4)
    got = gm.primary(win)
    require(got == 'lemon', f'S2 double-click on "lemon" in K3 selects it (PRIMARY {got!r})')
    win.away()
    print('ok: S1 S2 selection through the gaps')


def wait_done(win, n, label, timeout=20):
    until(lambda: len(q2.blocks(win)) > n and q2.blocks(win)[-1]['complete'], label, timeout)
    time.sleep(.6)


def alt_story(win, g):
    n = len(q2.blocks(win))
    if FAULT != 'alt':
        prog = win.w / 'prog.py'
        prog.write_text(gm.PROG)
        log = win.w / 'alt.log'
        win.type(f'python3 {prog} {log} 1049 1:2,1:9,1:20')
        win.key('Return')
        size = Path(str(log) + '.size')
        until(lambda: size.exists() and size.read_text().strip(), 'setup: the alternate-screen program started', 10)
        time.sleep(1)
    win.away()
    im = win.shot('alt')
    cells = marker_cells(im, (MAG, CYAN, YEL))
    found = sorted(cells[MAG] + cells[CYAN] + cells[YEL], key=lambda b: b[1])
    tops = [b[1] for b in found]
    require(len(found) == 3 and abs(tops[1] - tops[0] - 7 * CH) <= 1 and abs(tops[2] - tops[0] - 18 * CH) <= 1,
            f'A1 alternate screen: three cells {7 * CH} and {18 * CH} px apart (7 and 18 rows), a plain grid '
            f'(observed tops {tops})')
    require(42 <= tops[0] - CH <= 70, f'A1 alternate screen: grid origin y in 42..70 (observed {tops[0] - CH})')
    hits = any_hair(im, TOP + 1, tops[2] + CH)
    require(not hits, f'A1 alternate screen: no hairline (rows {hits})')
    win.type('q')
    wait_done(win, n, 'setup: the alternate-screen program ends')
    print('ok: A1 alternate screen draws the plain grid')


def report_story(win, g):
    n = len(q2.blocks(win))
    log = win.w / 'mrep.log'
    win.type(f'mrep {log}')
    win.key('Return')
    rowf = Path(str(log) + '.row')
    until(lambda: rowf.exists() and rowf.read_text().strip(), 'setup: mrep started', 10)
    time.sleep(1)
    r = int(rowf.read_text())
    rows = [x.strip() for x in q2.screen_rows(win)]
    require('target' in rows and rows.index('target') + 1 == r,
            f'setup: mrep\'s row {r} reads "target" on the owner API (rows {rows[:r + 2]})')
    require('kiwi lemon mango' in rows, 'setup: K3 still on the screen')
    r3 = rows.index('kiwi lemon mango') + 1
    win.away()
    im = win.shot('mrep')
    greens = marker_cells(im, (GRN, YEL))
    require(len(greens[GRN]) == 1 and len(greens[YEL]) == 1, f'setup: mrep\'s green cell and K3\'s cell on screen ({greens})')
    gy, yy = greens[GRN][0][1] + CH // 2, greens[YEL][0][1] + CH // 2
    if FAULT == 'naive':
        gy, yy = g['cmd1'] + (r - 1) * CH + CH // 2, g['cmd1'] + (r3 - 1) * CH + CH // 2
    seen = 0

    def read():
        nonlocal seen
        time.sleep(.7)
        got = [json.loads(l) for l in log.read_text().splitlines()] if log.exists() else []
        new, seen = ''.join(got[seen:]), len(got)
        return new
    for col, y, row, what in ((5, gy, r, 'mrep\'s row below the gaps'), (10, yy, r3, '"lemon" in K3')):
        win.click(col_x(g, col, 4), y)
        got = read()
        want = gm.sgr(0, col, row) + gm.sgr(0, col, row, 'm')
        require(got == want, f'R1 a click on {what} reports cell {col};{row}: {want!r} (got {got!r})')
    win.away()
    win.type('q')
    wait_done(win, n, 'setup: mrep ends')
    print('ok: R1 mouse reports through the gaps')


def wheel_story(win, g):
    b = q2.run(win, 'seq 1 80; mark g 0 0 lastrow', 'setup V1', timeout=12)
    win.away()
    time.sleep(.8)
    im = win.shot('v1')
    top_in = q2.hairline(im) or 874
    gr = sorted(marker_cells(im, (GRN,))[GRN], key=lambda b: b[1])
    require(gr and gr[-1][1] >= TOP + 1 and gr[-1][3] <= top_in,
            f'V1 the newest row (lastrow) is in view above the input at y {top_in} (green cells {gr})')
    win.x('mousemove', '--window', win.wid, '800', '400')
    found = None
    for i in range(80):
        win.x('click', '4')
        time.sleep(.35)
        im = win.shot('wheel')
        cells = marker_cells(im, (CYAN, YEL))
        if len(cells[CYAN]) == 1 and len(cells[YEL]) == 1 and cells[CYAN][0][1] >= 90 and cells[YEL][0][3] <= top_in - 5:
            found = (im, cells[CYAN][0], cells[YEL][0])
            break
    require(found, 'W1 wheel up brings K2 and K3 into view within 80 notches')
    t_last = time.time()
    im, c, y = found
    _, total = gap(im, c, y, LINE_BG, 5, 'W1 after wheel scrolling, K2 -> K3')
    require(abs(total - g['gap3']) <= 1, f'W1 the gap is the one measured at rest ({g["gap3"]} px, observed {total})')
    win.x('mousemove', '--window', win.wid, '800', '401')
    time.sleep(max(0, t_last + .2 - time.time()))
    q2.idle(win, 'W2 after wheel scrolling')
    win.x('mousemove', '--window', win.wid, '800', '400')
    back = False
    for i in range(80):
        win.x('click', '5')
        time.sleep(.3)
        if i % 4 == 3:
            gr = marker_cells(win.shot('wheel-back'), (GRN,))[GRN]
            if gr and max(b[3] for b in gr) <= top_in:
                back = True
                break
    require(back, 'W1 wheel down brings lastrow back into view')
    win.away()
    print('ok: V1 W1 W2 scrolling keeps the gaps')


def sticky_story(win, g):
    n = len(q2.blocks(win))
    win.type(': stickyjob; seq 1 150; sleep 12')
    win.key('Return')
    until(lambda: any(l.strip() == '150' for l in q2.screen_rows(win)), 'setup: 150 lines of output', 10)
    time.sleep(1.2)
    win.away()
    strip = (240, TOP, W, TOP + 46)

    def text(name):
        im = win.shot(name)
        return ' '.join(' '.join(w[0] for w in r) for r in readings(im, strip))
    a = text('sticky-1')
    m = re.search(r'running (\d+) ?s', a)
    require('stickyjob' in a and m, f'T1 the pane top reads "stickyjob" and "running N s" (read {a!r})')
    t = time.time()
    time.sleep(3)
    b = text('sticky-2')
    m2 = [int(v) for v in re.findall(r'running (\d+) ?s', b)]
    require(any(2 <= v - int(m.group(1)) <= 5 for v in m2), f'T1 the running time counts up ({a!r} -> {b!r})')
    time.sleep(1.2)
    frames = len(q1.frames_between(win, t, t + 4))
    require(2 <= frames <= 8, f'T1 a running block redraws about once a second (observed {frames} frames in 4 s)')
    wait_done(win, n, 'setup: stickyjob ends', 20)
    time.sleep(1)
    c = text('sticky-3')
    require('running' not in c, f'T1 "running" is gone after the block ends (read {c!r})')
    q2.idle(win, 'T2 after the sticky block ends')
    print('ok: T1 T2 sticky command line')


def main_story():
    fail_code = 0 if FAULT == 'fail' else 3
    win = Win('main', dict(BASE))
    try:
        start = setup_blocks(win, fail_code)
        g = geometry(win, start)
        if FAULT == 'idle':
            win.type('sleep 20')
            win.key('Return')
            time.sleep(1.5)
            win.away()
        q2.idle(win, 'I1 with three finished blocks on screen')
        print('ok: I1 0 idle frames')
        select_story(win, g)
        if FAULT == 'select':
            return
        alt_story(win, g)
        if FAULT == 'alt':
            return
        report_story(win, g)
        if FAULT == 'naive':
            return
        wheel_story(win, g)
        sticky_story(win, g)
    finally:
        win.close()


def plain_rows(win, label):
    im = win.shot(label)
    cells = marker_cells(im, (MAG, CYAN, YEL))
    found = sorted(cells[MAG] + cells[CYAN] + cells[YEL], key=lambda b: b[1])
    tops = [b[1] for b in found]
    require(len(found) == 3 and abs(tops[1] - tops[0] - 2 * CH) <= 1 and abs(tops[2] - tops[0] - 4 * CH) <= 1,
            f'{label}: output cells {2 * CH} and {4 * CH} px apart (rows 2 and 4), the plain grid (observed tops {tops})')
    require(42 <= tops[0] - CH <= 70, f'{label}: grid origin y in 42..70 (observed {tops[0] - CH})')


def classic_story():
    mode = 'editor' if FAULT == 'classic' else 'classic'
    win = Win('classic', dict(BASE, command_input=mode))
    try:
        for text, code in ((K1, 0), (k2(), 3), ('mark y 0 0 kiwi lemon mango', 0)):
            q2.run(win, text, f'setup C1 {text}', timeout=12)
        win.away()
        time.sleep(1)
        plain_rows(win, 'C1 classic')
        bl = q2.blocks(win)
        require(len(bl) >= 3 and [b.get('exit_code') for b in bl[-3:]] == [0, 3, 0],
                f'C1 classic: the three OSC 133 blocks exist ({[(q2.cmd(b), b.get("exit_code")) for b in bl]})')
    finally:
        win.close()
    print('ok: C1 classic draws the plain grid')


def noosc_story():
    shell = '/bin/bash' if FAULT == 'osc' else 'minish'
    win = Win('noosc', dict(BASE), shell=shell)
    try:
        for text in (K1, k2(), 'mark y 0 0 kiwi lemon mango'):
            win.type(text)
            win.key('Return')
            word = text.split()[-1]
            until(lambda: any(r.strip() == word or r.strip().endswith(' ' + word) and not r.strip().startswith(('mark', 'minish'))
                              for r in q2.screen_rows(win)), f'setup N1 {text}', 10)
        time.sleep(1)
        win.away()
        time.sleep(.6)
        plain_rows(win, 'N1 without OSC 133')
        require(not q2.blocks(win), f'N1 without OSC 133: no block ({[q2.cmd(b) for b in q2.blocks(win)]})')
    finally:
        win.close()
    print('ok: N1 no OSC 133 draws the plain grid')


def main():
    for tool in ('Xvfb', 'xdotool', 'xclip', 'ffmpeg', 'tesseract'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    stories = {None: [main_story, classic_story, noosc_story], 'classic': [classic_story], 'osc': [noosc_story]}
    for story in stories.get(FAULT, [main_story]):
        story()
    if FAULT is None:
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: Q4 breathing blocks')


if __name__ == '__main__':
    known = (None, 'fail', 'select', 'alt', 'naive', 'idle', 'classic', 'osc')
    try:
        require(FAULT in known, f'known fault {FAULT}')
        main()
    except Fail as e:
        print(f'FAIL: Q4 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: Q4 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
