#!/usr/bin/env python3
"""Menu gate: the right-click menu in a terminal pane, as in the mockup Marc approved on 2026-11-13
(R1 yes: keep Clanky's rows; R2 yes: "has the mouse" chip).

Usage: gate-menu.py [--fault copy|shift|link|reset|postai|over]
A fault changes the run's input on purpose; the gate must then FAIL. `over` needs CLANKY_BIN built with the
control-faults feature: it plants the window's own fault that draws the menu over its opener.

With CLANKY_BIN the same stories run against that window (clanky-gpui), which also writes its control record
(scripts/window-chrome/RECORD.md): K1 and K19 then read the menu's box and its opener from the record's
"context-menu" screen line instead of a screenshot difference, and K1 and K20 check the menu's rows by label. That
window has no "has the mouse" chip yet, so K10 is skipped there, and it has one pane, so K19 uses that pane's
bottom-right cell with nothing to hide first.

Runs in a real Clanky window on a private Xvfb display (the mouse gate's harness). Rows are reached by keyboard and
judged by what they do, not by how they are drawn.

Interface fixed by this gate:
  Rows, top to bottom (the mockup): [Open link, Copy link] only over a link; Copy, Paste, Select all;
  [Copy command, Copy output, Run again] only over a finished command block; Find…, Split right, Split below,
  Start an agent here…, Agent access; Reset terminal, Reset and clear; Close pane.
  Post-AI mode leaves out Start an agent here… and Agent access.
  Keys: the menu opens with no row highlighted. Down highlights the next row, Up the previous one; from no
  highlight, Down goes to the first row and Up to the last. Greyed rows (Copy with no selection) are skipped.
  Enter runs the highlighted row, Escape closes the menu.
  A plain right-click opens it unless the program has mouse reporting on; then the right-click goes to the program
  and Shift+right-click opens the menu. While a program has mouse reporting on, the pane's top bar says
  "has the mouse".

Expected results, fixed before the work:
  K1 a right-click opens the menu next to the pointer (the first changed pixel is within 40 px right of and below
     it) and keeps the selection: PRIMARY stays "beta" and its highlight is unchanged. From the record: the opener
     is the pointer (a zero-size box within 1 px of it), the menu's top-left is 0 to 40 px right of and below it and
     the menu does not hold the pointer, and its rows are Copy, Paste, Select all, Find…, Split right, Split below,
     Start an agent here…, Agent access, Reset terminal, Reset and clear, Close pane. That window's menu casts the
     rulebook's floating shadow (0 12 32, black at 35 %), which reaches the "beta" row above it, so there the
     highlight's fill must equal the earlier fill darkened by that shadow (scripts/floating/expected_floating.py's
     model, within its 3 grey levels) instead of being unchanged.
  K2 Escape closes it and the window is pixel-for-pixel as before the right-click.
  K3 Copy (Down, Enter) puts "beta" on the CLIPBOARD.
  K4 Paste (Down x2, Enter) sends ESC[200~PASTE-ME ESC[201~ to a program with bracketed paste on.
  K5 Select all (Down x3, Enter) puts the screen text on PRIMARY: it contains "alpha beta gamma /srv/devops/x-y.txt".
  K6 0 frames from 1 s to 4 s after the menu opens.
  K7 a left click outside closes the menu and does nothing else: the window is as before the right-click.
  K8 with nothing selected, Down skips the greyed Copy: Down, Enter pastes.
  K9 in mode 1002+1006 a plain right-click at 10;5 sends ESC[<2;10;5M ESC[<2;10;5m and opens no menu.
  K10 the pane's top bar reads "has the mouse" (tesseract) while the program has the mouse, and not after it exits.
  K11 Shift+right-click in that mode sends the program nothing and opens the menu.
  K12 over "https://example.invalid/clanky-menu": Open link (Down, Enter) runs xdg-open with exactly that address;
      Copy link (Down x2, Enter) puts exactly it on the CLIPBOARD.
  K13 over a finished OSC 133 block whose command is "echo menu-block-ok": Copy command (Down x3, Enter; Copy is
      greyed) copies "echo menu-block-ok"; Copy output (Down x4) copies "menu-block-ok" (one trailing newline
      allowed); Run again (Down x5) shows "menu-block-ok" on one more row.
  K14 after `printf '\033[?1000;1006h'` at the shell, Shift+right-click, Reset terminal (Down x8, Enter) turns
      mouse reporting off: a double-click on "resetword" selects it.
  K15 Reset and clear (Down x9, Enter) leaves no "resetword" on screen.
  K16 Split right (Down x4) and Split below (Down x5) each add one pane.
  K17 Agent access (Down x7) flips the pane's allow_agent from false to true, and again back to false.
  K18 Close pane (Up, Enter) removes the pane it was opened on.
  K19 opened at the window's bottom-right corner (rails hidden with Ctrl+Shift+B, bottom-right cell of the bottom
      pane), the menu flips to stay in the window: every changed pixel is left of and above the pointer (+2 px).
      From the record: the menu's box ends at most 2 px right of and below the pointer and starts inside the window.
  K20 Post-AI window: Down x7, Enter is Reset and clear (the agent rows are gone), so the screen is cleared. From
      the record, its rows are those of K1 without Start an agent here… and Agent access (Copy greyed, no selection).
"""
import importlib.util, json, os, re, shlex, shutil, subprocess, sys, time
from collections import Counter
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_MENU_WORK', '/srv/devops/build/clanky-menu-gate-run'))
os.environ['CLANKY_MOUSE_WORK'] = str(ROOT)
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
# The harness gives the window none of the CLANKY_ variables, so a wrapper sets the control record (in the window's
# own folder, its working directory) and, for the `over` fault, the window's planted fault.
WINDOW_BIN = os.environ.get('CLANKY_BIN')
WRAPPER = ROOT.with_name(ROOT.name + '-bin') / 'clanky-gpui'
if WINDOW_BIN:
    WRAPPER.parent.mkdir(parents=True, exist_ok=True)
    planted = 'CLANKY_CONTROL_FAULT=popover-over-opener ' if FAULT == 'over' else ''
    WRAPPER.write_text(f'#!/bin/sh\nCLANKY_CONTROL_RECORD="$PWD/controls.jsonl" {planted}'
                       f'exec {shlex.quote(WINDOW_BIN)} "$@"\n')
    WRAPPER.chmod(0o755)
    os.environ['CLANKY_BIN'] = str(WRAPPER)
spec = importlib.util.spec_from_file_location('gm', Path(__file__).with_name('gate-mouse.py'))
gm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gm)
Fail, require, until, Window, Pane = gm.Fail, gm.require, gm.until, gm.Window, gm.Pane
primary, set_clip, LINE, E = gm.primary, gm.set_clip, gm.LINE, gm.E
URL = 'https://example.invalid/clanky-menu'
OPENED = ROOT / 'xdg-open.log'


def _shot(self, name):
    """Screenshot without the pointer, so before/after pictures compare exactly."""
    path = self.w / f'{name}.png'
    subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-f', 'x11grab', '-draw_mouse', '0', '-video_size', '1400x900',
                    '-i', self.env['DISPLAY'] + '+0,0', '-frames:v', '1', str(path)], env=self.env, check=True,
                   stdin=subprocess.DEVNULL)
    from PIL import Image
    return Image.open(path).convert('RGB')


Window.shot = _shot


PANE = {}
ROWS = ['Copy', 'Paste', 'Select all', 'Find…', 'Split right', 'Split below', 'Start an agent here…', 'Agent access',
        'Reset terminal', 'Reset and clear', 'Close pane']
POST_AI_ROWS = [row for row in ROWS if row not in ('Start an agent here…', 'Agent access')]


def load_script(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def shadow_error(before, after, area, box):
    """The largest difference, in grey levels over the three channels, between the highlight's fill in `area` (x0, y0,
    x1, y1) after the menu opened and the same fill before it under the menu's shadow, as the rulebook's model draws
    it. The fill is the colour most pixels of the area had before; the glyphs on it are left out, their edges being
    drawn afresh with each frame."""
    floating = load_script('expected_floating', Path(__file__).with_name('floating') / 'expected_floating.py')
    shadow = floating.chrome.load_tokens()['shadows']['popover']
    cells = [(x, y) for y in range(area[1], area[3]) for x in range(area[0], area[2])]
    fill = Counter(before.getpixel(cell) for cell in cells).most_common(1)[0][0]
    worst = 0
    for x, y in (cell for cell in cells if before.getpixel(cell) == fill):
        alpha = floating.shadow_alpha((x + .5, y + .5), box, shadow, 1)
        worst = max(worst, *(abs(got - level * (1 - alpha)) for got, level in zip(after.getpixel((x, y)), fill)))
    return worst, floating.SHADOW_TOLERANCE_LEVELS


def menu_record(win):
    """The last "context-menu" screen line of the window's control record while the menu is open, or None."""
    path = win.w / 'controls.jsonl'
    last = None
    for line in path.read_text().splitlines() if path.exists() else []:
        entry = json.loads(line)
        if entry.get('record') == 'screen' and entry.get('screen') == 'context-menu':
            last = entry
    return last if last and last.get('popover') else None


def open_menu_record(win, what):
    return until(lambda: menu_record(win), f'{what}: the record shows the menu open', 3)


def labels(entry):
    return [c['label'] for c in entry['controls'] if c['kind'] == 'menu-item']


def check_opened_at(entry, px, py):
    """K1 from the record: opened by the pointer, beside it, not over it."""
    box, opener = entry['popover']['box'], entry['popover']['opener']
    require(opener[2:] == [0, 0] and abs(opener[0] - px) <= 1 and abs(opener[1] - py) <= 1,
            f'K1 the menu\'s opener is the pointer ({px},{py}): {opener}')
    covers = box[0] <= px < box[0] + box[2] and box[1] <= py < box[1] + box[3]
    require(px <= box[0] <= px + 40 and py <= box[1] <= py + 40 and not covers,
            f'K1 menu opens beside the pointer ({px},{py}), not over it: box {box}')
    require(labels(entry) == ROWS, f'K1 the menu\'s rows: {labels(entry)}')


def diff_box(a, b):
    """Bounding box (x0, y0, x1, y1), in window pixels, of pixels that differ between two screenshots inside the
    pane and its top bar, or None. The rest of the window is left out: the activity column's relative times change
    on their own."""
    from PIL import ImageChops
    x0, y0, cw, ch, cols, rows = PANE['p'].geo + (PANE['p'].cols, PANE['p'].rows)
    r = (max(0, int(x0) - 8), max(0, int(y0) - 40), int(x0 + cols * cw) + 8, int(y0 + rows * ch) + 8)
    box = ImageChops.difference(a.crop(r), b.crop(r)).getbbox()
    return box and (box[0] + r[0], box[1] + r[1], box[2] + r[0], box[3] + r[1])


def choose(win, downs=0, up=False, enter=True):
    for _ in range(downs):
        win.key('Down')
    if up:
        win.key('Up')
    if enter:
        win.key('Return')
    time.sleep(.5)


def menu(win, p, col, row, shift=False):
    p.click(col, row, 3, ('shift',) if shift else ())
    time.sleep(.4)


def screen_rows(win):
    lines = win.o.call(4, 'screen', tab_id=win.tab, history_offset=0, history_limit=1)['lines']
    return [l if isinstance(l, str) else l.get('text', '') for l in lines]


def find_cell(win, text):
    for i, line in enumerate(screen_rows(win)):
        if text in line:
            return line.index(text) + 1, i + 1
    raise Fail(f'"{text}" on screen')


def bar_text(win, p):
    """OCR of the strip above the pane's first row, up to the window's top: the title bar, where the chip is.
    Marc's R1 of 2026-11-14: the title bar is 46 px and the pane has 14 px of padding, so the first row starts at
    46 + 14 = 60 and the strip is the 60 px above it (was 40: the old 38 px bar and its pane padding)."""
    x0, y0, cw, ch = p.geo
    im = win.shot('bar').crop((max(0, int(x0) - 8), max(0, int(y0) - 60), int(x0 + p.cols * cw), int(y0) - 1))
    im = im.resize((im.width * 3, im.height * 3))
    path = win.w / 'bar.png'
    im.save(path)
    out = subprocess.run(['tesseract', str(path), '-', '--psm', '6'], capture_output=True, text=True, timeout=30)
    return out.stdout.lower()


def panes(win):
    return len(win.o.call(1, 'list_tabs')['tabs'])


def selection_story(win, p):
    p.start('2004')
    p.click(8, 3, repeat=2)
    require(primary(win) == 'beta', f'setup: double-click selects "beta": {primary(win)!r}')
    p.move(13, 3)
    a = win.shot('k-before')
    menu(win, p, 13, 3)
    b = win.shot('k-open')
    px, py = p.at(13, 3)
    opened = None
    if WINDOW_BIN:
        opened = open_menu_record(win, 'K1')
        check_opened_at(opened, px, py)
    else:
        box = diff_box(a, b)
        require(box and px - 4 <= box[0] <= px + 40 and py - 4 <= box[1] <= py + 40,
                f'K1 menu opens next to the pointer ({px},{py}): changed box {box}')
    require(primary(win) == 'beta', f'K1 right-click keeps the selection: PRIMARY {primary(win)!r}')
    bx0, by0 = p.at(7, 3, 0, 0); bx1, by1 = p.at(10, 3, 1, 1)
    if opened:
        worst, tolerance = shadow_error(a, b, (bx0, by0, bx1, by1), opened['popover']['box'])
        require(worst <= tolerance, f'K1 the "beta" highlight is unchanged under the menu\'s shadow: worst {worst:.1f} '
                                    f'grey levels off, at most {tolerance}')
    else:
        require(a.crop((bx0, by0, bx1, by1)).tobytes() == b.crop((bx0, by0, bx1, by1)).tobytes(),
                'K1 the "beta" highlight is unchanged while the menu is open')
    t0 = time.time()
    time.sleep(4.5)
    late = [t for t in win.frames() if t0 + 1 < t < t0 + 4]
    require(not late, f'K6 0 frames 1-4 s after the menu opens: {len(late)}')
    win.key('Escape'); time.sleep(.5)
    require(diff_box(a, win.shot('k-escaped')) is None, 'K2 Escape closes the menu and restores the window')
    set_clip(win, 'stale', 'clipboard')
    menu(win, p, 13, 3)
    choose(win, 2 if FAULT == 'copy' else 1)
    p.bytes()
    require(primary(win, 'clipboard') == 'beta', f'K3 Copy puts "beta" on CLIPBOARD: {primary(win, "clipboard")!r}')
    set_clip(win, 'PASTE-ME', 'clipboard')
    menu(win, p, 13, 3)
    choose(win, 2)
    got = p.bytes()
    require(got == f'{E}[200~PASTE-ME{E}[201~', f'K4 Paste sends the bracketed clipboard: got {got!r}')
    menu(win, p, 13, 3)
    choose(win, 3)
    require(LINE in (primary(win) or ''), f'K5 Select all puts the screen on PRIMARY: {(primary(win) or "")[:80]!r}')
    p.click(8, 3, repeat=2)
    p.move(13, 3)
    a = win.shot('k7-before')
    menu(win, p, 13, 3)
    p.click(5, 9)  # left of the menu, which covers about 252 x 320 px right of and below the pointer
    time.sleep(.4)
    require(diff_box(a, win.shot('k7-after')) is None, 'K7 a click outside closes the menu and changes nothing else')
    p.click(30, 9)  # nothing open now: this click clears the selection
    menu(win, p, 13, 3)
    choose(win, 1)
    got = p.bytes()
    require(got == f'{E}[200~PASTE-ME{E}[201~', f'K8 Down skips the greyed Copy and pastes: got {got!r}')
    p.stop()


def mouse_mode_story(win, p):
    p.start('1002;1006')
    p.move(10, 5)
    a = win.shot('k9-before')
    menu(win, p, 10, 5)
    got = p.bytes()
    want = f'{E}[<2;10;5M{E}[<2;10;5m'
    require(got == want, f'K9 plain right-click goes to the program: got {got!r}, want {want!r}')
    box = diff_box(a, win.shot('k9-after'))
    require(box is None, f'K9 no menu opens on a plain right-click: changed box {box}')
    if WINDOW_BIN:
        print('skipped: K10, this window has no "has the mouse" chip yet')
    else:
        require('has the mouse' in bar_text(win, p), 'K10 the pane bar reads "has the mouse" while mc-like programs have it')
    p.move(10, 5)
    a = win.shot('k11-before')
    menu(win, p, 10, 5, shift=FAULT != 'shift')
    got = p.bytes()
    require(got == '', f'K11 Shift+right-click sends the program nothing: got {got!r}')
    require(diff_box(a, win.shot('k11-open')) is not None, 'K11 Shift+right-click opens the menu')
    win.key('Escape')
    p.stop()
    time.sleep(.5)
    if not WINDOW_BIN:
        require('has the mouse' not in bar_text(win, p), 'K10 the pane bar no longer says "has the mouse" after it exits')


def link_story(win, p):
    win.type('clear; printf "%s%s\\n\\n" https://example.invalid/ clanky-menu\n')
    col, row = until(lambda: find_cell(win, URL), 'K12 address printed')
    target = (col + 5, row + 1) if FAULT == 'link' else (col + 5, row)
    menu(win, p, *target)
    choose(win, 1)
    time.sleep(.5)
    got = OPENED.read_text().strip() if OPENED.exists() else ''
    require(got == URL, f'K12 Open link runs xdg-open with the address: {got!r}')
    menu(win, p, col + 5, row)
    choose(win, 2)
    require(primary(win, 'clipboard') == URL, f'K12 Copy link copies the address: {primary(win, "clipboard")!r}')


def block_story(win, p):
    win.type("clear; printf '\\033]133;A\\007$ \\033]133;B\\007echo menu-block-ok\\033]133;C\\007\\r\\n"
             "menu-block-ok\\r\\n\\033]133;D;0\\007'\n")
    until(lambda: any(l.strip() == 'menu-block-ok' for l in screen_rows(win)), 'K13 block printed')
    col, row = next((1, i + 1) for i, l in enumerate(screen_rows(win)) if l.strip() == 'menu-block-ok')
    for downs, want in ((3, 'echo menu-block-ok'), (4, 'menu-block-ok')):
        set_clip(win, 'stale', 'clipboard')
        menu(win, p, col + 3, row)
        choose(win, downs)
        got = primary(win, 'clipboard') or ''
        require(got.rstrip('\n') == want and got.count('\n') <= 1, f'K13 block row {downs} copies {want!r}: {got!r}')
    before = sum(l.strip() == 'menu-block-ok' for l in screen_rows(win))
    menu(win, p, col + 3, row)
    choose(win, 5)
    until(lambda: sum(l.strip() == 'menu-block-ok' for l in screen_rows(win)) > before, 'K13 Run again runs it again', 8)


def reset_story(win, p):
    win.type("clear; printf 'reset%s\\n\\033[?1000;1006h' word\n")
    col, row = until(lambda: find_cell(win, 'resetword'), 'K14 word printed')
    time.sleep(.5)
    if FAULT != 'reset':
        menu(win, p, col + 2, row, shift=True)
        choose(win, 8)
    set_clip(win, 'none', 'primary')
    p.click(col + 2, row, repeat=2)
    require(primary(win) == 'resetword', f'K14 Reset terminal turns mouse reporting off: PRIMARY {primary(win)!r}')
    p.click(col + 20, row + 3)
    menu(win, p, col + 2, row)
    choose(win, 9)
    until(lambda: not any('resetword' in l for l in screen_rows(win)), 'K15 Reset and clear empties the screen', 5)


def flip_story(win, p):
    """At the window's real bottom-right corner: rails hidden, the bottom-right cell of the bottom pane (the window
    starts with two stacked panes; the gate's pane is the top one)."""
    if WINDOW_BIN:
        flip_in_one_pane(win, p)
        return
    win.key('ctrl+shift+b')
    time.sleep(.8)
    win.x('mousemove', '--window', win.wid, '700', '800', 'click', '1')
    time.sleep(.5)
    q = Pane(win)
    q.n = 100  # its own prog-*.log names; the top pane's prog-1.log.size says 12 x 89
    q.measure()
    PANE['p'] = q
    try:
        x, y = q.at(q.cols, q.rows)
        require(x > 1400 - 252 and y > 900 - 300, f'setup: the corner cell {x},{y} leaves no room right or below')
        q.move(q.cols, q.rows)
        a = win.shot('k19-before')
        menu(win, q, q.cols, q.rows)
        box = diff_box(a, win.shot('k19-open'))
        win.key('Escape')
        require(box and box[2] <= x + 2 and box[3] <= y + 2, f'K19 menu at the corner stays inside: box {box}, pointer {x},{y}')
    finally:
        PANE['p'] = p
        win.key('ctrl+shift+b')
        time.sleep(.8)
        p.click(40, 10)


def flip_in_one_pane(win, p):
    """K19 in a window of one pane, from the record: the menu opened at that pane's bottom-right cell."""
    x, y = p.at(p.cols, p.rows)
    require(x > 1400 - 252 and y > 900 - 300, f'setup: the corner cell {x},{y} leaves no room right or below')
    p.move(p.cols, p.rows)
    menu(win, p, p.cols, p.rows)
    box = open_menu_record(win, 'K19')['popover']['box']
    win.key('Escape')
    time.sleep(.4)
    require(box[0] >= 0 and box[1] >= 0 and box[0] + box[2] <= x + 2 and box[1] + box[3] <= y + 2,
            f'K19 menu at the corner stays inside: box {box}, pointer {x},{y}')


def layout_story(win, p):
    tab = win.tab
    require(win.o.call(3, 'permission_get', tab_id=tab)['allow_agent'] is False, 'setup: agent access starts off')
    for want in (True, False):
        menu(win, p, 20, 4)
        choose(win, 7)
        got = win.o.call(3, 'permission_get', tab_id=tab)['allow_agent']
        require(got is want, f'K17 Agent access flips to {want}: {got}')
    n = panes(win)
    menu(win, p, 20, 4)
    choose(win, 4)
    until(lambda: panes(win) == n + 1, f'K16 Split right adds a pane ({n} -> {panes(win)})', 6)
    menu(win, p, 3, 2)
    choose(win, 5)
    until(lambda: panes(win) == n + 2, f'K16 Split below adds a pane ({n + 1} -> {panes(win)})', 6)
    menu(win, p, 3, 2)
    choose(win, up=True)
    until(lambda: panes(win) == n + 1, f'K18 Close pane removes a pane ({n + 2} -> {panes(win)})', 6)


def post_ai_story():
    win = Window('postai', theme='dark', theme_dark='ink', font_family='IBM Plex Mono', font_size_pt=11,
                 cell_height_scale=1.25, cursor_blink='off', post_ai=FAULT != 'postai')
    try:
        p = Pane(win)
        p.measure()
        win.type('clear; echo postai-$((1+1))marker\n')
        col, row = until(lambda: find_cell(win, 'postai-2marker'), 'K20 marker printed')
        menu(win, p, col + 30, row + 2)
        if WINDOW_BIN:
            got = labels(open_menu_record(win, 'K20'))
            require(got == POST_AI_ROWS, f'K20 Post-AI menu rows: {got}')
        choose(win, 7)
        until(lambda: not any('postai-2marker' in l for l in screen_rows(win)), 'K20 Post-AI: Down x7 is Reset and clear', 5)
        win.key('Escape')
    finally:
        win.close()


def main():
    for tool in ('Xvfb', 'xdotool', 'xclip', 'ffmpeg', 'tesseract'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    (ROOT / 'bin').mkdir(parents=True)
    opener = ROOT / 'bin/xdg-open'
    opener.write_text(f'#!/bin/sh\nprintf "%s\\n" "$1" > {OPENED}\n')
    opener.chmod(0o755)
    os.environ['PATH'] = f'{ROOT / "bin"}:{os.environ["PATH"]}'
    win = Window('menu', theme='dark', theme_dark='ink', font_family='IBM Plex Mono', font_size_pt=11,
                 cell_height_scale=1.25, cursor_blink='off')
    try:
        p = Pane(win)
        PANE['p'] = p
        p.measure()
        selection_story(win, p)
        mouse_mode_story(win, p)
        flip_story(win, p)
        link_story(win, p)
        block_story(win, p)
        reset_story(win, p)
        layout_story(win, p)
    finally:
        win.close()
    post_ai_story()
    shutil.rmtree(ROOT)
    shutil.rmtree(WRAPPER.parent, ignore_errors=True)
    print('PASS: menu')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: {e}')
        sys.exit(1)
