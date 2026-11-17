#!/usr/bin/env python3
"""C2 gate: terminal colour schemes (design 2026-11-14, milestone C2; Marc's words: "about 40 well-made schemes (the
default palettes of popular terminals and the popular themes) plus import of other terminals' colour files", "Good
feel and good looks are more important than having a million features").

Usage: gate-c2-schemes.py [--fault dots|escape|follow|source|idle|import|bad]
With CLANKY_BIN (a one-pane window) only the grid colours run: see pane_colours_story.
A fault changes the run's input on purpose; a correct build must then FAIL.

Harness: the Q2 gate's window (scripts/gate-q2-input.py Win, itself the Q1 and T2 harness: free_display from :100 up),
a real Clanky window on a private Xvfb display, 1400 x 900 px at scale 1, software Vulkan, IBM Plex Mono 11 pt at
cell_height_scale 1.25, cursor blink off, theme Graphite dark, TZ=UTC, command_input "classic" (bash's own prompt,
so the terminal's own cursor sits at the prompt and the grid has no block gaps [Q4 C1]). The shell is real bash; the
gate's ~/.bashrc sources /etc/bashrc, puts ~/bin first on PATH and sets PS1='$ '. Screenshots leave out the pointer.
The config file is the harness's (win.config, read back with win.saved()), whatever its format.

Independent sources. [R] the iTerm2-Color-Schemes collection (MIT, https://github.com/mbadolato/iTerm2-Color-Schemes),
checked out at commit 95705098082c5d07fad152c31f020d50d38e10e8 (2026-11-13) in /srv/devops/build/ref/iTerm2-Color-Schemes
(sparse: schemes windowsterminal alacritty kitty ghostty wezterm konsole Xresources; the gate refuses another
commit). Its files are parsed by the gate's own Python parsers, never by Clanky: plistlib (.itermcolors, each
component round(c x 255)), json (Windows Terminal), tomllib (Alacritty, WezTerm), yaml.safe_load (Alacritty YAML),
configparser (Konsole, Ptyxis), and line splitting for Kitty, Ghostty and Xresources as those programs document
their formats. Its licenses.yml gives each scheme's licence. [T] assets/themes/graphite-dark.json (the window theme,
unchanged by this work). [V] VTE: SGR 40-47 and 100-107 paint a cell's background in palette colours 0-7 and 8-15,
SGR 7 swaps it to the default foreground, the block cursor is drawn in the cursor colour, a selected cell's
background is the selection colour; a scheme without a cursor colour draws the cursor in its foreground (as VTE's
default and as Clanky's Ptyxis import already does). [Q1] the Q1 gate's theme picker: four dots of ansi 1, 2, 4, 3,
6-12 px wide, centres 14 px (+-2) apart, on a chip of the row's background (scripts/gate-q1-look.py dots_ok).

Interface fixed by this gate.
  Built-in schemes ship in assets/terminal-schemes/ (any file layout) with assets/terminal-schemes/SOURCES.md: a
  Markdown table whose header row names the columns Scheme | Source | Commit | Licence (in that order), one row per
  built-in scheme. Source is a URL; Commit is 7-40 hex digits; Licence is an SPDX id from MIT, MIT OR Apache-2.0,
  Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, 0BSD, Zlib, CC0-1.0. A Source inside iTerm2-Color-Schemes names the
  scheme's file (".../<dir>/<Name>.<ext>", %20 for spaces) and its Licence must equal licenses.yml's entry for
  <Name> [R]: an "unknown", "nonfree" or GPL entry there may not be taken from the collection.
  terminal_colors gains "selection" (#rrggbb), next to background, foreground, cursor and ansi (16).
  Settings > Terminal has a group whose heading contains "Colour scheme" (or "Color scheme"), with a search field
  that reads "Search" while empty, then the list: first the row "Follow the window theme" (chosen while the file
  has no terminal_colors; a click on it removes terminal_colors), then the schemes, each row its name and four dots
  as the theme picker [Q1]. Typing in the focused search field keeps the rows whose name contains the text (case
  ignored). Down/Up move a highlight through the rows shown and preview it in the live terminal without writing the
  file; Enter keeps the highlighted scheme; Escape puts the saved colours back without writing. A click on a row
  keeps it. Keeping a scheme writes terminal_colors with all 20 of its colours (background, foreground, cursor,
  selection, ansi), lowercase #rrggbb, and leaves every other key as it was.
  The same group has a button whose label starts with "Import"; it opens a path field (as today's Ptyxis import);
  Enter imports that file. `clanky import-colors [--config PATH] FILE` imports FILE into the config file (PATH, else
  the default one) and exits 0; a running window picks the change up as it does hand edits. A file it cannot read as
  a colour scheme makes it exit non-zero and leaves the config file byte for byte. Formats, told apart by content or
  extension: iTerm2 .itermcolors, Windows Terminal JSON scheme, Alacritty TOML and YAML, Kitty .conf, Ghostty,
  WezTerm TOML, Konsole .colorscheme, Xresources, Ptyxis .palette ([Dark] section). An imported value the file
  does not give (Konsole and Xresources have no selection colour) is left to Clanky; the gate checks only the
  colours the file gives, and the cursor as the foreground when the file has no cursor [V].

Expected results, fixed before the work (colour tolerance: +-2 per channel on screen, +-1 in the file):
  W0 setup: `clear; swatch` prints row 0: a 24-bit (13,247,169) marker cell, 16 cells with SGR 40..47, 100..107 (palette
     0..15), one SGR 7 cell (the foreground); row 1: "[" ten spaces "]"; row 2 is bash's "$ " with the cursor at
     column 2. The marker cell's box gives the cell size; cell (c, r) is sampled at its centre.
  F0 with no terminal_colors the swatch is Graphite's [T]: background #111111 at cell (60, 0), ansi 0..15 the theme's
     ansi list, the SGR 7 cell its foreground #fafafa.
  P1 preview: on the Colour scheme group, typing "tokyo" into the search field and Down makes the pane's background
     (cell (60, 0)) one of the TokyoNight*.itermcolors backgrounds [R] (#1a1b26, #24283b, #e1e2e7, #222436), the
     file byte for byte unchanged; a second Down a different one of them; Up the first again; Escape puts Graphite's
     #111111 and ansi 1 back on screen, the file still unchanged.
  P2 Down, Down, Enter: terminal_colors equals the TokyoNight* file whose background the second Down showed, all
     20 colours [R]; the other keys unchanged.
  F1 a click on "Follow the window theme" removes terminal_colors; the swatch is Graphite again (F0).
  K1 picker rows [Q1]: for each sample below, after typing its search text, its row has four dots of its ansi 1, 2,
     4, 3 [R] on a chip of its background.
  K2 a click on the row writes terminal_colors equal to the collection file [R] (20 colours) and, with Settings
     closed, the window draws them: background, ansi 0..15, foreground (SGR 7 cell), cursor (cell (2, 2)) and the
     selection (cell (5, 1) while "[ ... ]" is selected by a drag).
     Samples (search text: the row's words; collection file in schemes/):
       "nord": Nord (not Light, Wave, fox) -> Nord.itermcolors (MIT)
       "storm": Tokyo Night Storm -> TokyoNight Storm.itermcolors (Apache-2.0)
       "gruvbox": the row with Gruvbox and Light (not Hard, Material) -> Gruvbox Light.itermcolors (MIT)
       "dawn": Rose Pine Dawn -> Rose Pine Dawn.itermcolors (MIT)
       "kanagawa": Kanagawa (not Dragon, Lotus, bones) -> Kanagawa Wave.itermcolors (MIT)
       "ghostty": Ghostty (Ghostty's default) -> Ghostty Default Style Dark.itermcolors (MIT)
  I1 0 frames from 1 s to 3.5 s after the last sample settles (the 0-idle-frames rule).
  M1 the Import button with schemes/Tomorrow Night Eighties.itermcolors: terminal_colors and the window's colours
     equal the file [R].
  M2 `clanky import-colors --config <the window's file> FILE` for each format, in this order; each exits 0 within
     15 s, the file's terminal_colors equals FILE's colours, every other key is unchanged, and within 4 s the
     window draws them (background, ansi 0..15, foreground, cursor; selection when FILE gives one):
       Windows Terminal  windowsterminal/Gruvbox Dark.json
       Alacritty TOML    alacritty/Catppuccin Mocha.toml
       Alacritty YAML    gate-written alacritty.yml with the colours of alacritty/Material.toml, values as '0xrrggbb'
       Kitty             kitty/Rose Pine Moon.conf
       Ghostty           ghostty/Kanagawa Dragon
       WezTerm           wezterm/Everforest Dark Hard.toml
       Konsole           konsole/Solarized Osaka Night.colorscheme (no cursor: cursor = foreground)
       Xresources        Xresources/Tomorrow Night Bright
       Ptyxis            gate-written lagoon.palette, [Dark]: Background #0f2a2e, Foreground #e6e1cf, Cursor #ffcc66,
                         Color0..15 the LAGOON list below; [Light] a different set that must not be used
       iTerm2            schemes/Tomorrow Night.itermcolors
     Each file's background differs from the one before it, so a file that is not picked up shows.
  M3 a file that is not a colour scheme (plain text) makes import-colors exit non-zero; the config file is unchanged.
  I2 0 frames from 1 s to 3.5 s after the last import.
  S1 SOURCES.md: at least 36 rows, each with a URL, a 7-40 hex commit and a licence from the list above; collection
     rows agree with licenses.yml [R]; and a row for each scheme Marc's list names (regex on the Scheme cell, case
     ignored): xterm; gnome|ptyxis; konsole; campbell; mac ?os|terminal\\.app|apple; iterm2?; alacritty; kitty;
     wezterm; ghostty; dracula; solarized dark; solarized light; gruvbox dark; gruvbox light; nord; tokyo ?night
     (night)? alone; tokyo ?night storm; tokyo ?night day; one dark; one light; ros[eé] ?pine (main)? alone;
     ros[eé] ?pine moon; ros[eé] ?pine dawn; everforest dark; everforest light; kanagawa; monokai; github dark;
     github light; ayu; material; night ?owl; oceanic next; tomorrow night; zenburn.
Planted faults: dots (expects the dots in ansi order 1, 2, 3, 4), escape (Enter instead of Escape in P1), follow
(the Follow row is not clicked in F1), source (Nord's expected colours parsed from Nord Light.itermcolors), idle (a
background loop prints a counter every 0.3 s during I1), import (the Ghostty import's expected colours parsed from
ghostty/Kanagawa Wave instead of Kanagawa Dragon), bad (M3 is given a valid Kitty file).
"""
import configparser, importlib.util, json, os, plistlib, re, shutil, subprocess, sys, time, tomllib
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(os.environ.get('CLANKY_C2_WORK', '/srv/devops/build/clanky-c2-gate-run'))
os.environ['CLANKY_Q2_WORK'] = str(ROOT)
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
_argv = sys.argv
sys.argv = sys.argv[:1]  # the harness modules read their own --fault from argv; they get none
spec = importlib.util.spec_from_file_location('q2', Path(__file__).with_name('gate-q2-input.py'))
q2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q2)
sys.argv = _argv
q1, t2 = q2.q1, q2.t2
Fail, require, until, near, centre = q1.Fail, q1.require, q1.until, q1.near, q1.centre
REPO = Path(__file__).resolve().parents[1]
W, H = 1400, 900
REF = Path('/srv/devops/build/ref/iTerm2-Color-Schemes')
REF_COMMIT = '95705098082c5d07fad152c31f020d50d38e10e8'
MARK = (13, 247, 169)
GRAPHITE = json.loads((REPO / 'assets/themes/graphite-dark.json').read_text())
BASE = dict(q1.BASE, theme='dark', theme_dark='graphite-dark', command_input='classic')
BASHRC = '[ -f /etc/bashrc ] && . /etc/bashrc\nPATH="$HOME/bin:$PATH"\nPS1=\'$ \'\n'
SWATCH = r'''#!/bin/sh
# swatch: a marker cell, palette 0..15 as cell backgrounds, one reverse-video cell; then "[" ten spaces "]"
printf '\033[48;2;13;247;169m \033[0m'
for i in 0 1 2 3 4 5 6 7; do printf '\033[4%sm \033[0m' "$i"; done
for i in 0 1 2 3 4 5 6 7; do printf '\033[10%sm \033[0m' "$i"; done
printf '\033[7m \033[0m\n[          ]\n'
'''
LAGOON = ['#0f2a2e', '#e05561', '#8cc265', '#d18f52', '#4aa5f0', '#c162de', '#42b3c2', '#d7dae0',
          '#4f6b72', '#ff616e', '#a5e075', '#f0a45d', '#4dc4ff', '#de73ff', '#4cd1e0', '#e6e6e6']
LAGOON_LIGHT = ['#fafafa', '#a01020', '#106010', '#806000', '#103090', '#702080', '#106070', '#202020',
                '#909090', '#c02030', '#208020', '#a08000', '#2050c0', '#9030a0', '#2090a0', '#000000']
LAGOON_FILE = ('[Palette]\nName=Gate Lagoon\nPrimary=false\n\n[Dark]\nBackground=#0f2a2e\nForeground=#e6e1cf\n'
               'Cursor=#ffcc66\n' + ''.join(f'Color{i}={c}\n' for i, c in enumerate(LAGOON)) +
               '\n[Light]\nBackground=#fafafa\nForeground=#202020\nCursor=#202020\n' +
               ''.join(f'Color{i}={c}\n' for i, c in enumerate(LAGOON_LIGHT)))
PERMISSIVE = {'mit', 'mit or apache-2.0', 'apache-2.0', 'bsd-2-clause', 'bsd-3-clause', 'isc', '0bsd', 'zlib', 'cc0-1.0'}
REQUIRED = [r'\bxterm\b', r'gnome|ptyxis', r'konsole', r'campbell', r'mac ?os|terminal\.app|apple', r'\biterm2?\b',
            r'alacritty', r'kitty', r'wezterm', r'ghostty', r'dracula', r'solarized dark', r'solarized light',
            r'gruvbox dark', r'gruvbox light', r'\bnord\b', r'^tokyo ?night( night)?$', r'tokyo ?night storm',
            r'tokyo ?night day', r'one dark', r'one light', r'^ros[eé] ?pine( main)?$', r'ros[eé] ?pine moon',
            r'ros[eé] ?pine dawn', r'everforest dark', r'everforest light', r'kanagawa', r'monokai', r'github dark',
            r'github light', r'\bayu\b', r'material', r'night ?owl', r'oceanic next', r'tomorrow night', r'zenburn']
SAMPLES = [('nord', {'nord'}, {'light', 'wave', 'fox', 'nordfox'}, 'Nord'),
           ('storm', {'storm'}, set(), 'TokyoNight Storm'),
           ('gruvbox', {'gruvbox', 'light'}, {'hard', 'material'}, 'Gruvbox Light'),
           ('dawn', {'dawn'}, set(), 'Rose Pine Dawn'),
           ('kanagawa', {'kanagawa'}, {'dragon', 'lotus', 'kanagawabones', 'bones'}, 'Kanagawa Wave'),
           ('ghostty', {'ghostty'}, {'light'}, 'Ghostty Default Style Dark')]


def rgb(h):
    h = h.strip().lower()
    h = h[2:] if h.startswith('0x') else h.lstrip('#')
    require(re.fullmatch(r'[0-9a-f]{6}', h), f'setup: colour {h!r} is #rrggbb')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def hexs(c):
    return '#%02x%02x%02x' % c


def scheme(bg, fg, cursor, selection, ansi):
    require(len(ansi) == 16 and all(ansi), f'setup: 16 ansi colours parsed ({ansi})')
    return dict(background=bg, foreground=fg, cursor=cursor, selection=selection, ansi=list(ansi))


# ---------------------------------------------------------------- independent parsers of the source files

def parse_iterm(path):
    d = plistlib.loads(Path(path).read_bytes())
    c = lambda k: tuple(round(d[k][n] * 255) for n in ('Red Component', 'Green Component', 'Blue Component')) if k in d else None
    return scheme(c('Background Color'), c('Foreground Color'), c('Cursor Color'), c('Selection Color'),
                  [c(f'Ansi {i} Color') for i in range(16)])


def parse_wt(path):
    d = json.loads(Path(path).read_text())
    names = ['black', 'red', 'green', 'yellow', 'blue', 'purple', 'cyan', 'white']
    ansi = [rgb(d[n]) for n in names] + [rgb(d['bright' + n[0].upper() + n[1:]]) for n in names]
    g = lambda k: rgb(d[k]) if k in d else None
    return scheme(g('background'), g('foreground'), g('cursorColor'), g('selectionBackground'), ansi)


ALA = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white']


def from_alacritty(d):
    c = d['colors']
    hexok = lambda v: isinstance(v, str) and re.fullmatch(r'(#|0x)[0-9a-fA-F]{6}', v.strip())
    g = lambda sec, k: rgb(c[sec][k]) if sec in c and hexok(c[sec].get(k)) else None
    ansi = [rgb(c['normal'][n]) for n in ALA] + [rgb(c['bright'][n]) for n in ALA]
    return scheme(g('primary', 'background'), g('primary', 'foreground'), g('cursor', 'cursor'),
                  g('selection', 'background'), ansi)


def parse_alacritty_toml(path):
    return from_alacritty(tomllib.loads(Path(path).read_text()))


def parse_alacritty_yaml(path):
    import yaml
    return from_alacritty(yaml.safe_load(Path(path).read_text()))


def parse_kitty(path):
    kv = {}
    for line in Path(path).read_text().splitlines():
        f = line.split()
        if len(f) >= 2 and not line.lstrip().startswith('#'):
            kv[f[0]] = f[1]
    g = lambda k: rgb(kv[k]) if k in kv and kv[k] != 'none' else None
    return scheme(g('background'), g('foreground'), g('cursor'), g('selection_background'),
                  [g(f'color{i}') for i in range(16)])


def parse_ghostty(path):
    kv, pal = {}, {}
    for line in Path(path).read_text().splitlines():
        if '=' not in line or line.lstrip().startswith('#'):
            continue
        k, v = (s.strip() for s in line.split('=', 1))
        if k == 'palette':
            n, c = v.split('=', 1)
            pal[int(n)] = rgb(c)
        else:
            kv[k] = v
    g = lambda k: rgb(kv[k]) if k in kv else None
    return scheme(g('background'), g('foreground'), g('cursor-color'), g('selection-background'),
                  [pal.get(i) for i in range(16)])


def parse_wezterm(path):
    c = tomllib.loads(Path(path).read_text())['colors']
    g = lambda k: rgb(c[k]) if k in c else None
    return scheme(g('background'), g('foreground'), g('cursor_bg'), g('selection_bg'),
                  [rgb(v) for v in c['ansi'] + c['brights']])


def parse_konsole(path):
    cp = configparser.ConfigParser(interpolation=None)
    cp.read_string(Path(path).read_text())
    g = lambda sec: tuple(int(v) for v in cp[sec]['Color'].split(',')[:3])
    return scheme(g('Background'), g('Foreground'), None, None,
                  [g(f'Color{i}') for i in range(8)] + [g(f'Color{i}Intense') for i in range(8)])


def parse_xresources(path):
    kv = {}
    for line in Path(path).read_text().splitlines():
        m = re.match(r'^\s*[\w.*-]*?[.*](color\d+|background|foreground|cursorColor)\s*:\s*(#[0-9a-fA-F]{6})\s*$', line)
        if m:
            kv[m.group(1)] = rgb(m.group(2))
    return scheme(kv.get('background'), kv.get('foreground'), kv.get('cursorColor'), None,
                  [kv.get(f'color{i}') for i in range(16)])


def parse_ptyxis(path):
    cp = configparser.ConfigParser(interpolation=None)
    cp.optionxform = str
    cp.read_string(Path(path).read_text())
    d = cp['Dark']
    return scheme(rgb(d['Background']), rgb(d['Foreground']), rgb(d['Cursor']) if 'Cursor' in d else None, None,
                  [rgb(d[f'Color{i}']) for i in range(16)])


def ref(rel):
    p = REF / rel
    require(p.is_file(), f'setup: {p} exists (sparse checkout of the collection)')
    return p


# ---------------------------------------------------------------- the window

def make_home(home):
    home.mkdir(mode=0o700, exist_ok=True)
    (home / '.bashrc').write_text(BASHRC)
    b = home / 'bin'
    b.mkdir(exist_ok=True)
    (b / 'swatch').write_text(SWATCH)
    (b / 'swatch').chmod(0o755)


q2.make_home = make_home
Win = q2.Win


def marker(im):
    import numpy as np
    a = np.asarray(im).astype(int)
    mask = np.abs(a - np.array(MARK)).max(axis=2) <= 2
    ys, xs = np.where(mask)
    if not len(ys):
        return None
    box = (int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1)
    cw, ch = box[2] - box[0], box[3] - box[1]
    if not (6 <= cw <= 30 and 12 <= ch <= 60 and mask.sum() >= .9 * cw * ch):
        return None
    return box


def cell(box, c, r):
    cw, ch = box[2] - box[0], box[3] - box[1]
    return box[0] + c * cw + cw // 2, box[1] + r * ch + ch // 2


def swatch(win):
    win.type('clear; swatch')
    win.key('Return')
    until(lambda: [l.rstrip() for l in q2.screen_rows(win)][1:3] == ['[          ]', '$'],
          f'setup W0: the swatch on rows 0-1 and the prompt on row 2 (rows {q2.screen_rows(win)[:4]})', 10)
    time.sleep(.8)
    win.away()
    box = marker(win.shot('w0'))
    require(box, 'setup W0: the swatch marker cell is on screen')
    return box


def drawn(win, box, label, select=False):
    """The colours on screen: background, ansi, foreground, cursor and (with select) the selection."""
    if select:
        y = cell(box, 0, 1)[1]
        cw = box[2] - box[0]
        win.x('mousemove', '--window', win.wid, str(box[0] + 2), str(y))
        time.sleep(.15)
        win.x('mousedown', '1')
        time.sleep(.2)
        win.x('mousemove', '--window', win.wid, str(box[0] + 6 * cw), str(y))
        time.sleep(.15)
        win.x('mousemove', '--window', win.wid, str(box[0] + 12 * cw - 2), str(y))
        time.sleep(.2)
        win.x('mouseup', '1')
        time.sleep(.5)
    win.away()
    im = win.shot(label)
    now = marker(im)
    require(now and abs(now[0] - box[0]) <= 1 and abs(now[1] - box[1]) <= 1,
            f'{label}: the swatch is where it was ({now} vs {box})')
    px = lambda c, r: im.getpixel(cell(box, c, r))
    out = dict(background=px(60, 0), ansi=[px(1 + i, 0) for i in range(16)], foreground=px(17, 0), cursor=px(2, 2))
    if select:
        out['selection'] = px(5, 1)
        win.click(*cell(box, 60, 1))  # a click clears the selection
        win.away()
    return out


def screen_ok(got, want, label):
    bad = []
    for k in ('background', 'foreground', 'cursor', 'selection'):
        if k not in got:
            continue
        w = want.get(k) or (want['foreground'] if k == 'cursor' else None)
        if w is not None and not near(got[k], w, 2):
            bad.append(f'{k} {got[k]} want {w}')
    bad += [f'ansi {i} {g} want {w}' for i, (g, w) in enumerate(zip(got['ansi'], want['ansi'])) if not near(g, w, 2)]
    return bad


def screen_check(win, box, want, label, select=False, timeout=4):
    end = time.time() + timeout
    while True:
        got = drawn(win, box, label, select)
        bad = screen_ok(got, want, label)
        if not bad or time.time() > end:
            break
        time.sleep(.4)
    require(not bad, f'{label}: the window draws the scheme ({"; ".join(bad[:5])})')


def file_ok(tc, want):
    if not isinstance(tc, dict):
        return [f'terminal_colors {tc!r}']
    bad = []
    try:
        for k in ('background', 'foreground', 'cursor', 'selection'):
            w = want.get(k)
            if w is not None and not (isinstance(tc.get(k), str) and near(rgb(tc[k]), w, 1)):
                bad.append(f'{k} {tc.get(k)} want {hexs(w)}')
        a = tc.get('ansi') or []
        if len(a) != 16:
            bad.append(f'ansi {a}')
        else:
            bad += [f'ansi {i} {g} want {hexs(w)}' for i, (g, w) in enumerate(zip(a, want['ansi'])) if not near(rgb(g), w, 1)]
    except Fail as e:
        bad.append(str(e))
    return bad


def file_check(win, want, label, others, timeout=3):
    try:
        until(lambda: not file_ok(win.saved().get('terminal_colors'), want), 'x', timeout)
    except Fail:
        raise Fail(f'{label}: terminal_colors in the config file equals the scheme '
                   f'({"; ".join(file_ok(win.saved().get("terminal_colors"), want)[:5])})')
    rest = {k: v for k, v in win.saved().items() if k != 'terminal_colors'}
    require(rest == others, f'{label}: every other key unchanged ({rest} vs {others})')


GRAPHITE_WANT = dict(background=rgb(GRAPHITE['colors']['background']), foreground=rgb(GRAPHITE['colors']['foreground']),
                     cursor=None, selection=None, ansi=[rgb(c) for c in GRAPHITE['ansi']])


def graphite_check(win, box, label):
    end = time.time() + 4
    while True:
        got = drawn(win, box, label)
        got.pop('cursor')
        bad = screen_ok(got, GRAPHITE_WANT, label)
        if not bad or time.time() > end:
            break
        time.sleep(.4)
    require(not bad, f'{label}: the swatch shows Graphite [T] ({"; ".join(bad[:5])})')


# ---------------------------------------------------------------- reading Settings

PANEL = (240, 48, 1260, 872)


def words(im, box=PANEL, faint=False):
    """Text read more than one way: faint 12-14 px labels misread at one psm or scale and not at another, so the
    words of every reading are kept (the Q1 gate's B4 and K3 rule); the expected words do not change."""
    out = q1.ocr_words(im, box, 3, 11) + q1.ocr_words(im, box, 3, 6)
    if faint:
        out += [(t, box[0] + a, box[1] + b, box[0] + c, box[1] + d) for t, a, b, c, d in q2.faint_words(im, box, 3)]
    return out


def settings_open(win):
    t = {w[0] for w in words(win.shot('open?'))}
    return 'appearance' in t and ('general' in t or 'keys' in t)


def close_settings(win):
    for _ in range(4):
        if not settings_open(win):
            return
        win.key('Escape')
        time.sleep(.5)
    require(not settings_open(win), 'setup: Escape closes Settings')


def open_schemes(win):
    """Settings > Terminal, scrolled to the Colour scheme group; returns the search field's word box."""
    if not settings_open(win):
        win.key('ctrl+comma')
        time.sleep(1)
    ws = words(win.shot('nav'))
    nav = [w for w in ws if w[0] == 'terminal' and w[1] < 700]
    require(nav, f'Settings shows a "Terminal" page in its nav (read {sorted({w[0] for w in ws})[:40]})')
    win.click(*centre(min(nav, key=lambda w: w[1])))
    time.sleep(.8)
    for _ in range(40):
        ws = words(win.shot('scheme'), faint=True)
        head = [w for w in ws if re.fullmatch(r'schemes?', w[0]) and any(
            re.fullmatch(r'colou?r', v[0]) and abs(centre(v)[1] - centre(w)[1]) <= 8 and v[3] < w[1] for v in ws)]
        if head and centre(head[0])[1] < 560:
            hy = centre(head[0])[1]
            search = [w for w in ws if w[0].startswith('search') and hy < centre(w)[1] < hy + 120]
            require(search, f'the Colour scheme group has a search field reading "Search" below its heading '
                            f'(read near it: {[w[0] for w in ws if hy - 10 < centre(w)[1] < hy + 120]})')
            return search[0], hy
        win.x('mousemove', '--window', win.wid, '820', '460', 'click', '5')
        time.sleep(.3)
    raise Fail('Settings > Terminal has a "Colour scheme" group (heading not found while scrolling 40 notches)')


def search_for(win, text):
    field, hy = open_schemes(win)
    win.click(*centre(field))
    win.key('End')
    win.x('key', '--repeat', '40', '--delay', '15', 'BackSpace')
    if text:
        win.type(text)
    time.sleep(.8)
    return field, hy


def find_row(win, field, want, avoid):
    im = win.shot('rows')
    ws = [w for w in words(im) if centre(w)[1] > centre(field)[1] + 12]
    for w in sorted(ws, key=lambda w: w[2]):
        if not any(t in w[0] for t in want):
            continue
        line = {v[0] for v in ws if abs(centre(v)[1] - centre(w)[1]) <= 8}
        if all(any(t in v for v in line) for t in want) and not any(a in v for v in line for a in avoid):
            # The dots start right of the name's last word. OCR also reads the dot chip as a word, so the rightmost
            # word on the line may be the chip itself: take the rightmost word that is part of the name (as the Q1
            # gate passes the matched name word to dots_ok).
            name = max((v for v in ws if abs(centre(v)[1] - centre(w)[1]) <= 8 and any(t in v[0] for t in want)),
                       key=lambda v: v[3])
            return im, w, name
    return im, None, None


# ---------------------------------------------------------------- stories

def pane_bg(win, box, label):
    return drawn(win, box, label)['background']


def preview_story(win, box):
    tokyo = {f.name: parse_iterm(f) for f in sorted((REF / 'schemes').glob('TokyoNight*.itermcolors'))}
    bgs = {s['background'] for s in tokyo.values()}
    require(len(tokyo) == 5, f'setup: five TokyoNight files in the collection ({list(tokyo)})')
    search_for(win, 'tokyo')
    before = win.config.read_bytes()
    win.key('Down')
    time.sleep(.6)
    a = pane_bg(win, box, 'p1-down')
    require(any(near(a, b, 2) for b in bgs) and win.config.read_bytes() == before,
            f'P1 Down previews a Tokyo Night scheme (background {a}, one of {sorted(bgs)}) without writing the file')
    win.key('Down')
    time.sleep(.6)
    b = pane_bg(win, box, 'p1-down2')
    require(any(near(b, x, 2) for x in bgs) and not near(a, b, 2) and win.config.read_bytes() == before,
            f'P1 a second Down previews another Tokyo Night scheme ({a} -> {b}) without writing the file')
    win.key('Up')
    time.sleep(.6)
    c = pane_bg(win, box, 'p1-up')
    require(near(c, a, 2) and win.config.read_bytes() == before, f'P1 Up previews the first again ({c} vs {a})')
    win.key('Return' if FAULT == 'escape' else 'Escape')
    time.sleep(.8)
    require(win.config.read_bytes() == before, 'P1 Escape writes nothing (the config file changed)')
    graphite_check(win, box, 'P1 Escape puts the saved colours back')
    print('ok: P1 Up/Down preview, Escape reverts')
    if FAULT == 'escape':
        return
    others = {k: v for k, v in win.saved().items() if k != 'terminal_colors'}
    search_for(win, 'tokyo')
    win.key('Down')
    time.sleep(.5)
    win.key('Down')
    time.sleep(.6)
    b = pane_bg(win, box, 'p2-down2')
    win.key('Return')
    match = [s for s in tokyo.values() if near(s['background'], b, 2)]
    require(match, f'P2 the second Down previews a Tokyo Night scheme ({b})')
    try:
        until(lambda: any(not file_ok(win.saved().get('terminal_colors'), s) for s in match), 'x', 3)
    except Fail:
        raise Fail(f'P2 Enter keeps the previewed scheme: terminal_colors equals it '
                   f'({"; ".join(file_ok(win.saved().get("terminal_colors"), match[0])[:5])})')
    rest = {k: v for k, v in win.saved().items() if k != 'terminal_colors'}
    require(rest == others, f'P2 every other key unchanged ({rest} vs {others})')
    print('ok: P2 Enter keeps the scheme')


def follow_story(win, box):
    field, hy = search_for(win, '')
    ws = [w for w in words(win.shot('follow')) if centre(w)[1] > centre(field)[1] + 12]
    fol = [w for w in ws if w[0] == 'follow' and any('window' in v[0] and abs(centre(v)[1] - centre(w)[1]) <= 8 for v in ws)]
    require(fol, f'F1 the list starts with "Follow the window theme" (read {[w[0] for w in ws][:20]})')
    if FAULT != 'follow':
        win.click(*centre(fol[0]))
    try:
        until(lambda: 'terminal_colors' not in win.saved(), 'x', 3)
    except Fail:
        raise Fail(f'F1 a click on Follow the window theme removes terminal_colors '
                   f'({win.saved().get("terminal_colors")})')
    close_settings(win)
    graphite_check(win, box, 'F1 Follow the window theme')
    print('ok: F1 follow the window theme')


def samples_story(win, box):
    for term, want, avoid, name in SAMPLES:
        src = 'Nord Light' if FAULT == 'source' and name == 'Nord' else name
        s = parse_iterm(ref(f'schemes/{src}.itermcolors'))
        others = {k: v for k, v in win.saved().items() if k != 'terminal_colors'}
        field, hy = search_for(win, term)
        im, w, row_end = find_row(win, field, want, avoid)
        require(w, f'K1 searching "{term}" shows the {name} row')
        order = (1, 2, 3, 4) if FAULT == 'dots' else (1, 2, 4, 3)
        problem = q1.dots_ok(im, row_end, [s['ansi'][i] for i in order], s['background'], min(W - 4, int(row_end[3]) + 360))
        require(not problem, f'K1 the {name} row has dots of ansi 1, 2, 4, 3 on its background: {problem}')
        win.click(*centre(w))
        file_check(win, s, f'K2 {name}', others)
        close_settings(win)
        screen_check(win, box, s, f'K2 {name}', select=True)
        print(f'ok: K1 K2 {name}')
    if FAULT == 'idle':
        win.type("(for i in $(seq 60); do printf '\\033[s\\033[1;80H%d\\033[u' $i; sleep .3; done) &")
        win.key('Return')
        time.sleep(1)
        win.away()
    q1.idle(win, 'I1 after the last scheme is kept')
    print('ok: I1 0 idle frames')


def import_button(win, box, path, want):
    others = {k: v for k, v in win.saved().items() if k != 'terminal_colors'}
    open_schemes(win)
    imp = []
    for _ in range(20):
        ws = words(win.shot('import'))
        head = [centre(w)[1] for w in ws if re.fullmatch(r'schemes?', w[0])]
        imp = sorted((w for w in ws if w[0].startswith('import') and (not head or centre(w)[1] > min(head))),
                     key=lambda w: w[2])
        if imp:
            break
        win.x('mousemove', '--window', win.wid, '820', '460', 'click', '5')
        time.sleep(.3)
    require(imp, 'M1 the Colour scheme group has an Import button')
    win.click(*centre(imp[0]))
    time.sleep(.6)
    win.key('End')
    win.x('key', '--repeat', '200', '--delay', '5', 'BackSpace')
    win.type(str(path))
    win.key('Return')
    file_check(win, want, 'M1 Import button, .itermcolors', others)
    close_settings(win)
    screen_check(win, box, want, 'M1 Import button, .itermcolors', select=True)
    print('ok: M1 Import button')


def cli(win, *args):
    return subprocess.run([str(q2.APP), 'import-colors', '--config', str(win.config), *map(str, args)], env=win.env,
                          capture_output=True, text=True, timeout=15, stdin=subprocess.DEVNULL)


def imports_story(win, box):
    work = win.w / 'imports'
    work.mkdir()
    material = tomllib.loads(ref('alacritty/Material.toml').read_text())
    yml = {'colors': {sec: {k: '0x' + v.lstrip('#').lower() for k, v in vals.items()}
                      for sec, vals in material['colors'].items()}}
    import yaml
    (work / 'alacritty.yml').write_text(yaml.safe_dump(yml, sort_keys=False))
    (work / 'lagoon.palette').write_text(LAGOON_FILE)
    button = ref('schemes/Tomorrow Night Eighties.itermcolors')
    import_button(win, box, button, parse_iterm(button))
    ghost = 'ghostty/Kanagawa Wave' if FAULT == 'import' else 'ghostty/Kanagawa Dragon'
    files = [('Windows Terminal', ref('windowsterminal/Gruvbox Dark.json'), parse_wt(ref('windowsterminal/Gruvbox Dark.json'))),
             ('Alacritty TOML', ref('alacritty/Catppuccin Mocha.toml'), parse_alacritty_toml(ref('alacritty/Catppuccin Mocha.toml'))),
             ('Alacritty YAML', work / 'alacritty.yml', parse_alacritty_yaml(work / 'alacritty.yml')),
             ('Kitty', ref('kitty/Rose Pine Moon.conf'), parse_kitty(ref('kitty/Rose Pine Moon.conf'))),
             ('Ghostty', ref('ghostty/Kanagawa Dragon'), parse_ghostty(ref(ghost))),
             ('WezTerm', ref('wezterm/Everforest Dark Hard.toml'), parse_wezterm(ref('wezterm/Everforest Dark Hard.toml'))),
             ('Konsole', ref('konsole/Solarized Osaka Night.colorscheme'), parse_konsole(ref('konsole/Solarized Osaka Night.colorscheme'))),
             ('Xresources', ref('Xresources/Tomorrow Night Bright'), parse_xresources(ref('Xresources/Tomorrow Night Bright'))),
             ('Ptyxis', work / 'lagoon.palette', parse_ptyxis(work / 'lagoon.palette')),
             ('iTerm2', ref('schemes/Tomorrow Night.itermcolors'), parse_iterm(ref('schemes/Tomorrow Night.itermcolors')))]
    require(parse_ptyxis(work / 'lagoon.palette')['ansi'] == [rgb(c) for c in LAGOON], 'setup: the Ptyxis file reads [Dark]')
    prev = parse_iterm(button)['background']
    # The setup reads the real files: the import fault changes only what M2 Ghostty expects, so it must reach M2.
    real = {'Ghostty': parse_ghostty(ref('ghostty/Kanagawa Dragon'))}
    for label, path, want in files:
        want = real.get(label, want)
        # A stale screen still shows the previous background exactly; the screen check allows +-2 per channel, so
        # a background more than 4 away from the previous one cannot pass on a stale screen.
        require(not near(want['background'], prev, 4), f'setup M2: {label} background differs from the one before')
        prev = want['background']
    for label, path, want in files:
        others = {k: v for k, v in win.saved().items() if k != 'terminal_colors'}
        r = cli(win, path)
        require(r.returncode == 0, f'M2 {label}: clanky import-colors exits 0 (exit {r.returncode}: '
                                   f'{(r.stderr or r.stdout).strip()[-200:]!r})')
        file_check(win, want, f'M2 {label}', others)
        screen_check(win, box, want, f'M2 {label}', select=want.get('selection') is not None)
        print(f'ok: M2 {label}')
    bad = work / 'notes.conf'
    if FAULT == 'bad':
        shutil.copy(ref('kitty/Rose Pine Moon.conf'), bad)
    else:
        bad.write_text('shopping list\nmilk\nbread\n')
    before = win.config.read_bytes()
    r = cli(win, bad)
    require(r.returncode != 0 and win.config.read_bytes() == before,
            f'M3 a plain-text file is refused (exit {r.returncode}) and the config file is unchanged '
            f'({"unchanged" if win.config.read_bytes() == before else "changed"})')
    print('ok: M3 a file that is not a scheme is refused')
    q1.idle(win, 'I2 after the last import')
    print('ok: I2 0 idle frames')


def main_story():
    win = Win('main', dict(BASE))
    try:
        box = swatch(win)
        graphite_check(win, box, 'F0 the default follows the window theme')
        print('ok: W0 F0 swatch, Graphite by default')
        preview_story(win, box)
        if FAULT == 'escape':
            return
        follow_story(win, box)
        if FAULT == 'follow':
            return
        samples_story(win, box)
        if FAULT in ('dots', 'source', 'idle'):
            return
        imports_story(win, box)
    finally:
        win.close()


def sources_story():
    import yaml
    lic = yaml.safe_load((REF / 'licenses.yml').read_text())
    path = REPO / 'assets/terminal-schemes/SOURCES.md'
    require(path.is_file(), f'S1 {path.relative_to(REPO)} exists')
    rows, header = [], None
    for line in path.read_text().splitlines():
        if not line.strip().startswith('|'):
            continue
        cells = [c.strip() for c in line.strip().strip('|').split('|')]
        if all(re.fullmatch(r':?-+:?', c) for c in cells):
            continue
        if header is None:
            header = [c.lower() for c in cells]
            require(header[:4] in (['scheme', 'source', 'commit', 'licence'], ['scheme', 'source', 'commit', 'license']),
                    f'S1 the table columns are Scheme | Source | Commit | Licence (observed {header})')
            continue
        rows.append(cells)
    require(len(rows) >= 36, f'S1 at least 36 schemes (observed {len(rows)})')
    for c in rows:
        require(len(c) >= 4, f'S1 row {c}: four cells')
        name, url, commit, licence = c[:4]
        url = url.strip('<>')
        m = re.search(r'https?://\S+', url)
        require(m, f'S1 {name}: a source URL ({url!r})')
        require(re.fullmatch(r'[0-9a-f]{7,40}', commit.strip('`')), f'S1 {name}: a 7-40 hex commit ({commit!r})')
        require(licence.lower() in PERMISSIVE, f'S1 {name}: a permissive licence ({licence!r})')
        if 'iTerm2-Color-Schemes' in m.group(0):
            base = unquote(m.group(0).rstrip(')').rsplit('/', 1)[-1])
            stem = re.sub(r'\.(itermcolors|json|toml|yml|yaml|conf|colorscheme)$', '', base)
            entry = lic.get(stem)
            require(entry, f'S1 {name}: the collection file {base!r} is in licenses.yml')
            require(str(entry.get('license', '')).lower() == licence.lower(),
                    f'S1 {name}: licence {licence} agrees with licenses.yml ({entry.get("license")})')
    names = [c[0].lower().replace('-', ' ').replace('é', 'e') for c in rows]
    missing = [r for r in REQUIRED if not any(re.search(r.replace('[eé]', 'e'), n) for n in names)]
    require(not missing, f'S1 a scheme for each name Marc listed (missing {missing})')
    print(f'ok: S1 SOURCES.md ({len(rows)} schemes)')


def pane_colours_story():
    """K2's drawing half, for a one-pane window (CLANKY_BIN): a window started with each sample's terminal_colors,
    written from the gate's own parse of the collection file, draws background, ansi 0..15, foreground, cursor and
    selection. The Settings picker, preview, import and live reload (P1, P2, F1, K1, M1-M3) need Settings."""
    win = None
    try:
        for term, want, avoid, name in SAMPLES:
            src = 'Nord Light' if FAULT == 'source' and name == 'Nord' else name
            s = parse_iterm(ref(f'schemes/{src}.itermcolors'))
            expected = parse_iterm(ref(f'schemes/{name}.itermcolors'))
            colours = dict(background=hexs(s['background']), foreground=hexs(s['foreground']),
                           cursor=hexs(s['cursor'] or s['foreground']), ansi=[hexs(c) for c in s['ansi']])
            if s['selection']:
                colours['selection'] = hexs(s['selection'])
            if win:
                win.close()
            win = Win(f'k2-{term}', dict(BASE, terminal_colors=colours))
            box = swatch(win)
            screen_check(win, box, expected, f'K2 {name}', select=True)
            print(f'ok: K2 {name} drawn in the pane')
        q1.idle(win, 'I1 after the last scheme is drawn')
        print('ok: I1 0 idle frames')
    finally:
        # A failed check must not leave its window, server and display running.
        if win:
            win.close()


def main():
    for tool in ('Xvfb', 'xdotool', 'xclip', 'ffmpeg', 'tesseract', 'git'):
        require(shutil.which(tool), f'tool {tool} installed')
    head = subprocess.run(['git', '-C', str(REF), 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
    require(head == REF_COMMIT, f'setup: {REF} is at {REF_COMMIT} (observed {head or "no checkout"})')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    if t2.ONE_PANE:
        print('note: one-pane window: K2 drawing only; left out W0-F0 (theme default), P1, P2, F1, K1, M1-M3, I2, S1')
        pane_colours_story()
        print('PASS: C2 terminal colours in the pane')
        return
    main_story()
    if FAULT is None:
        sources_story()
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: C2 terminal colour schemes')


if __name__ == '__main__':
    known = (None, 'dots', 'escape', 'follow', 'source', 'idle', 'import', 'bad')
    try:
        require(FAULT in known, f'known fault {FAULT}')
        main()
    except Fail as e:
        t2.gate_app.print_window_errors()
        print(f'FAIL: C2 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        t2.gate_app.print_window_errors()
        print(f'FAIL: C2 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
