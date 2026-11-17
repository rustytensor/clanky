#!/usr/bin/env python3
"""Q1 gate: a quieter Clanky with the feel of Warp (design 2026-11-13, milestone Q1 "quiet look";
mockup Marc approved: /srv/devops/build/clanky-redesign/mockup.html, sections 1, 2, 4 and 5; his answers R1 yes,
R2 Horizontal, R3 yes).

Usage: gate-q1-look.py [--fault dots|rails|preview|exit|idle|tools]
A fault changes the run's input on purpose; a correct build must then FAIL.

Every check runs in a real Clanky window on a private Xvfb display from :100 up (the T2 harness's free_display),
1400 x 900 px at scale 1, software Vulkan, IBM Plex Mono 11 pt at cell_height_scale 1.25 (9 x 26 px cells, measured
from Ptyxis/VTE in the T2 gate), cursor blink off, TZ=UTC, SHELL=/bin/sh. Screenshots leave out the pointer
(ffmpeg -draw_mouse 0). The window starts with two stacked panes; the gate closes the second one first through the
owner API, so every window below has one terminal pane unless a step adds more. Cell geometry is measured from two
magenta cells (the mouse gate's Pane.measure), never assumed. Text on screen is read with tesseract.

Interface fixed by this gate (config.xml settings; strings, as cursor_blink and agent_pane_access are):
  tab_style      "horizontal" (default, Marc's R2) | "vertical"
  tools_panel    "on" (default) | "off". The old boolean "rails" stays readable: when tools_panel is absent,
                 rails false means off and rails true means on; when both are present tools_panel wins. Settings
                 removes "rails" from the file the first time it writes tools_panel, in the same write.
  tools          list of distinct ids from "files", "recent", "agents"; default ["files","recent","agents"].
                 Clanky writes it in that order.
  command_input  "editor" (default, Marc's R1) | "classic". Q1 stores it; Q2 makes "editor" do something.
  block_style    "quiet" (default) | "cards" (cards = today's command cards)
  status_line    "show" (default) | "hide"
  Any other value of these keys (another string, a boolean, a number, an unknown tool id) is a config error: the
  window refuses to start, as it does today for a bad cursor_blink. Theme keys do not change: the picker writes
  theme ("system" | "dark" | "light"), theme_dark, theme_light, and terminal_colors for a Ptyxis palette.
  Settings writing back a default may remove the key instead; the gate accepts the key absent or set to the default.
  Ctrl+, opens Settings; its 780 x 600 frame and nav stay as today (Appearance is the second nav row, at window
  (410, 258) in a 1400 x 900 window). Settings > Appearance starts with the Layout group (mockup section 4), then
  the Theme group (section 5). Segments are labelled exactly Horizontal | Vertical, On | Off, Editor | Classic,
  Quiet | Cards, Show | Hide, the tool chips Files, Recent, Agents. A click on a segment or chip saves at once.
  Theme picker: "Follow the system" On | Off (On = theme "system"; Off = theme "dark" or "light"). With Follow off,
  one list in the mockup's order: Graphite, Ink, Stone, Paper, Catppuccin Mocha, Catppuccin Macchiato,
  Catppuccin Frappe, AI Bedazzled, Unicorn Vomit (dark), then Graphite, Stone, Paper, Catppuccin Latte,
  Unicorn Tears (light), then imported Ptyxis palettes. With Follow on, two columns "In dark mode" (left) and
  "In light mode" (right), Ptyxis palettes after them. On the Appearance page Down moves the highlight one row
  (the first Down goes to the row after the saved theme) and previews it in the window behind Settings without
  writing config.xml; Escape puts the saved theme back, still without writing; Enter saves the highlighted theme.
  A click on a row saves it. Ptyxis palettes are read from $HOME/.local/share/org.gnome.Ptyxis/palettes/*.palette;
  saving one writes terminal_colors exactly as `clanky terminal-colors import` prints its [Dark] section.
  The tools panel footer has the segments Files, Recent, Agents (only the tools in "tools"). The Agents view has
  one access row per terminal pane reading "off" or "read + run"; a click on the row flips that pane's access, as
  today's right-column row does. Tab titles are the folder name of the tab's focused pane.
  OSC 133 blocks: the gate prints the marks itself (A prompt, B command, C output, D;exit), as gate-menu K13 does.

Expected results, fixed before the work. Sources: [M] the approved mockup's CSS (explicit px and colours),
[T] assets/themes/*.json, [P] the T2 gate's Ptyxis/VTE cell measurement (9 x 26 px), [X] xterm/sh behaviour.
Colours are Graphite dark [T]: sidebar #080808, background #111111, card #111111, muted #1f1f1f,
border #ffffff1a (over #080808 = (33,33,33), over #111111 = (41,41,41)), ok #7cc48f, err #ec8a7c, agent #dfae74.
Graphite err equals the mockup's --g-err, so the failed-block hairline rgba(236,138,124,.45) [M] over #111111 is
(116,72,65).
  P1 a config with every Q1 key at a non-default value (vertical, off, ["files"], classic, cards, hide) starts the
     window, and its pane has 124-125 columns: vertical tab column 236 px [M .vtabs, same width as .tools], pane
     padding 18 px each side [M .blk], 9 px cells [P]: floor((1400-236-36)/9) = 125, one column of slack.
  P2 each bad value (tab_style "diagonal", tools_panel true, tools ["files","bogus"], command_input "fancy",
     block_style "loud", status_line 1) never stops the window (Marc's Y2 2026-11-14): the window opens with the
     last good values, which at a cold start are the default settings (docs/config.md), not the rest of the file,
     and is still running 3 s later and after Settings. Defaults: the same column count as a reference window
     started on an empty config.xml (no setting at all), and the status line shown in the default theme's colour:
     theme "system" under GSETTINGS_BACKEND=memory is dark, theme_dark "stone-dark", sidebar #0f0d0b, so (700,890)
     is (15,13,11) +-3 [T] (the file asks for Graphite, sidebar (8,8,8)). Ctrl+, opens Settings and the part of the
     window it changed reads "line 13": config.xml holds the XML declaration (line 1), <clanky version="1">
     (line 2), the 8 BASE settings and theme "dark", theme_dark "graphite-dark" (lines 3..12), then the bad value
     on line 13.
  P3 config {} (no Q1 key): the defaults; 124-125 columns (tools panel 236 px [M .body grid 236px]) and the status
     line shown (pixel (700,890) is the sidebar colour (8,8,8), +-3).
  L1 {"rails": false}: tools panel off, 151-152 columns (pane padding 14 each side, Marc's sizes of 2026-11-14:
     floor(1372/9) = 152). L2 {"rails": false, "tools_panel": "on"}: 124-125 columns.
  G1 default window: grid origin x = 236 + 8 + 14 = 258 (+-3) [M tools panel 236; Marc's sizes of 2026-11-14: 8 px
     between the tools panel and the pane, 14 px pane padding], grid origin y in 42..70 (title bar 46 + padding 14
     = 60 with those sizes; no 40 px pane header), 124-125 columns, the last row ends above the status line (y <= 874).
  G2 pixels [M, T]: title bar (900,4) and (900,33) sidebar (8,8,8); no line under it, Marc's Y1 of 2026-11-14 (the
     Ptyxis look): its last row (900,45) of the 46 px bar is (8,8,8); tools panel (120,450) (8,8,8), no divider
     after it (Y1): its last column (235,450) and the 8 px gap after it (240,450) are the window colour (8,8,8);
     pane (300,450) (17,17,17); the old
     right column is gone: (1300,450) (17,17,17); status line (700,890) (8,8,8), its top hairline (700,874)
     (33,33,33). Tolerance 4.
  G3 the status line reads "Session 1", "1 tab", "server up" and "masking" [M .status].
  I1 0 frames from 1 s to 3.5 s after the window settles (the 0-idle-frames rule), with the file tree, the status
     line and a finished block on screen.
  B1 three blocks "echo quietok" (exit 0), "false quietbad" (exit 3), "echo quietok2" (exit 0, 2.3 s): no hairline
     above the first; a 1-2 px hairline (116,72,65) +-8 at the top edge of the failed block's command row; a 1-2 px
     hairline (41,41,41) +-5 above the third [M .blk border-top, .blk.fail].
  B2 the command row is bold: "quietok" in "$ echo quietok" has at least 1.3 x the ink of the output "quietok"
     (the T2 gate's bold rule) [M .cmd font-weight 600].
  B3 err-coloured text only on the failed block's command row, and it reads "exit 3" [M .meta .exit].
  B4 the third block's meta text (on its command row or the row above it) reads the folder "alpha" (~/alpha),
     the start time as HH:MM in UTC, and the duration "N.N s" with N.N in 2.0..3.5 [M .meta].
  H1 hovering the third block shows one floating bar: the changed pixels (any channel >= 15) form one box at most
     40 px tall whose top is 6 px (+-3) below the block's hairline and whose right edge is 14 px (+-3) from the
     window's right edge [M .hoverbar right 14, top 6]; it reads "Copy output", "Copy command", "Run again",
     "Ask agent". Copy output puts "quietok2" on the CLIPBOARD (one trailing newline allowed), Copy command
     "echo quietok2", Run again prints "quietok2" once more.
  S1 a running block "longjob" whose command row has scrolled off (150 lines of output): the top of the pane reads
     "longjob" and "running N s"; 3 s later N is 2-5 higher; the window draws 2-8 frames in those 4 s (a 1 s tick,
     not 30 frames a second); after the block ends, "running" is gone and I1 holds again.
  F1 tools panel, Files: in ~/alpha (folders assets, crates, docs; files notes.txt, Cargo.toml, README.md) it shows
     "alpha" and the six names, every folder above every file. After `cd ~/bravo` it shows kilo and mike.txt
     within 5 s and no "crates"; a file november.txt created from outside appears within 4 s; then I1 holds.
  T1 horizontal tabs: after Ctrl+Shift+T and `cd ~/alpha` the title bar reads "bravo" and "alpha"; the active tab
     has the Ptyxis fill, white at 8 % over the bar (Marc's Y1 2026-11-14): 8 + 0.08 x (255 - 8) = 27.8, so
     (28,28,28) +-3 just left of its title, the other does not. A click on "bravo"
     makes its group active (owner workspace_get). The first mark right of the inactive last tab's title is "+":
     a click adds a group. The first mark right of the active tab's title (within 30 px) is its close mark: a click
     removes that group and its terminal.
  T2 state dots [M .tab .dot]: while a block runs in a tab, the title bar has a 4-8 px blob of ok (124,196,143)
     +-12 left of that tab's title; after it ends, none. A tab started by `clanky agent --tab Fake DIR` gets a 4-8 px
     blob of agent (223,174,116) +-12.
  T3 the search box, 318 x 28 (Marc's Y1 2026-11-14, the wider box; was [M .search 260 x 26]): its card-coloured
     inside is 313-319 px wide and 24-28 px tall (the same -5..+1 and -4..0 px as before), it reads
     "search"; a click on it opens the palette: typing "Split right" and Enter adds a pane. The rightmost mark in
     the title bar is the settings gear: a click opens Settings ("Appearance" on screen). The leftmost mark is the
     tools toggle: a click adds 26-27 columns (236 / 9), a second click takes them back; Ctrl+Shift+B does the same.
  T4 tools panel footer: "Recent" shows a view that reads "command" (Command finished / failed) and no file names;
     "Agents" shows "access" and one row per pane; a click on an "off" row flips exactly one pane to allow_agent true.
  E1 Settings > Appearance > Layout, each click saved at once (config.xml within 3 s, same window process, no
     restart) and applied at once: Vertical -> tab_style "vertical", 26-27 fewer columns (a second 236 px column),
     the left column reads the tab's folder "charlie" and its last command "vtlastcmd", the title bar no longer
     reads "charlie"; Horizontal -> columns as before. Tools Off on a file with "rails": true -> tools_panel "off"
     and no "rails" key, 26-27 more columns; On -> as before, still no "rails". Chip Recent -> tools
     ["files","agents"] and no "Recent" in the panel footer; again -> all three. Classic -> command_input
     "classic"; Editor -> default. Cards -> block_style "cards" and the block's pixels change; Quiet -> the same
     pixels as before. Hide -> status_line "hide", exactly one more row (26 px = one 26 px cell), (700,890) is the
     pane colour; Show -> rows as before.
  E2 0 frames from 1 s to 3 s after the last Settings click.
  K1 theme picker preview, from {"theme":"dark","theme_dark":"stone-dark","theme_light":"paper-light"}: Down
     changes the window pixel (1300,450) (Stone background (20,18,16) [T]) and leaves config.xml byte for byte;
     Escape puts the pixel back (+-1) and the bytes are still the same. Down, Enter writes theme_dark "paper-dark";
     with Settings closed (1300,450) is Paper's background (31,28,23) +-2 [T].
  K2 Follow the system On writes theme "system" and keeps theme_dark "paper-dark" and theme_light "paper-light";
     the page then reads "dark mode" and "light mode".
  K3 colour dots [M .th i, T]: on every row (14 built-in themes in their columns, and the gate's Ptyxis palette
     "Harbor Quiet"), along the row's middle there are four dots of ansi[1], ansi[2], ansi[4], ansi[3] (+-6) in
     that order from left to right, each 6-12 px wide, centres 14 px apart (+-2) [M: 5.5, 19.5, 33.5, 47.5], with the
     chip colour (the theme's own background, or the palette's Background) between the first two (+-4).
     Harbor Quiet's dots are #d04040, #40c060, #4080e0, #e0c040 on #102030 (its Color1, Color2, Color4, Color3).
  K4 a click on Harbor Quiet writes terminal_colors equal to its [Dark] section (background, foreground, cursor,
     ansi Color0..15, lowercase) and leaves the theme keys; with Settings closed the empty pane is #102030 +-2.
Planted faults: dots (expects ansi 1,2,3,4 order), rails (L1 starts with rails true), preview (Enter instead of
Escape in K1), exit (the failed block exits 0), idle (a running block on screen during I1), tools (the Recent chip
is not clicked in E1).
"""
import importlib.util, json, os, re, shutil, subprocess, sys, time
from pathlib import Path
import importlib.util as _config_xml_util
_config_xml_spec = _config_xml_util.spec_from_file_location('clanky_config_xml', Path(__file__).with_name('clanky_config_xml.py'))
config_xml = _config_xml_util.module_from_spec(_config_xml_spec)
_config_xml_spec.loader.exec_module(config_xml)  # config.xml writer and reader shared by the gate harnesses

ROOT = Path(os.environ.get('CLANKY_Q1_WORK', '/srv/devops/build/clanky-q1-gate-run'))
os.environ['CLANKY_MOUSE_WORK'] = str(ROOT)
spec = importlib.util.spec_from_file_location('gm', Path(__file__).with_name('gate-mouse.py'))
gm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gm)
t2 = gm.t2
Fail, require, until, Pane = gm.Fail, gm.require, gm.until, gm.Pane
APP = t2.APP
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
REPO = Path(__file__).resolve().parents[1]
W, H = 1400, 900
SIDEBAR, BG, MUTED, LINE_SIDE, LINE_BG = (8, 8, 8), (17, 17, 17), (31, 31, 31), (33, 33, 33), (41, 41, 41)
OK, ERR, AGENT, FAIL_LINE = (124, 196, 143), (236, 138, 124), (223, 174, 116), (116, 72, 65)
NAV_APPEARANCE = (410, 258)
TAB_ON = (28, 28, 28)  # white at 8 % over the bar (8,8,8): 8 + 0.08 x 247 = 27.8 (Marc's Y1 2026-11-14)
BASE = dict(restore_layout=False, startup_animation=False, history=False, animations=False, cursor_blink='off',
            font_family='IBM Plex Mono', font_size_pt=11, cell_height_scale=1.25)
HARBOR = {'background': '#102030', 'foreground': '#d8dee9', 'cursor': '#f0f0f0',
          'ansi': ['#102030', '#d04040', '#40c060', '#e0c040', '#4080e0', '#a060c0', '#40b0b0', '#c0c0c0',
                   '#506070', '#e06060', '#60e080', '#f0d060', '#60a0f0', '#c080e0', '#60d0d0', '#f0f0f0']}
HARBOR_FILE = ('[Palette]\nName=Harbor Quiet\nPrimary=false\n\n[Dark]\nBackground=#102030\nForeground=#d8dee9\n'
               'Cursor=#f0f0f0\n' + ''.join(f'Color{i}={c}\n' for i, c in enumerate(HARBOR['ansi'])) +
               '\n[Light]\nBackground=#f0f0f0\nForeground=#102030\nCursor=#102030\n' +
               ''.join(f'Color{i}={c}\n' for i, c in enumerate(HARBOR['ansi'])))
DARK_ROWS = [('graphite', 'graphite-dark'), ('ink', 'ink'), ('stone', 'stone-dark'), ('paper', 'paper-dark'),
             ('mocha', 'catppuccin-mocha'), ('macchiato', 'catppuccin-macchiato'), ('frapp', 'catppuccin-frappe'),
             ('bedazzled', 'ai-bedazzled'), ('vomit', 'unicorn-vomit')]
LIGHT_ROWS = [('graphite', 'graphite-light'), ('stone', 'stone-light'), ('paper', 'paper-light'),
              ('latte', 'catppuccin-latte'), ('tears', 'unicorn-tears')]


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def near(a, b, tol=4):
    return all(abs(x - y) <= tol for x, y in zip(a, b))


def diff(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


class Win(t2.Window):
    """The T2 harness window with the exact config given (no implied rails), stderr kept, the pointer left out of
    screenshots, and the second startup pane closed."""

    def __init__(self, name, cfg, extra_env=None):
        self.w = ROOT / name
        self.w.mkdir(mode=0o700)
        for d in ('home', 'config', 'data', 'state', 'runtime'):
            (self.w / d).mkdir(mode=0o700)
        self.procs, self.server, self.p = [], None, None
        try:
            self._start(cfg, extra_env)
        except BaseException:
            self.close()
            raise

    def _start(self, cfg, extra_env):
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(('XDG_', 'CLANKY', 'WAYLAND', 'DBUS', 'GNOME', 'GSETTINGS'))}
        env.update(HOME=str(self.w / 'home'), XDG_CONFIG_HOME=str(self.w / 'config'), XDG_DATA_HOME=str(self.w / 'data'),
                   XDG_STATE_HOME=str(self.w / 'state'), XDG_RUNTIME_DIR=str(self.w / 'runtime'), SHELL='/bin/sh',
                   TZ='UTC', GSETTINGS_BACKEND='memory', WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1',
                   LP_NUM_THREADS='8', VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json',
                   CLANKY_FRAME_TRACE=str(self.w / 'frames.csv'), **(extra_env or {}))
        self.home = self.w / 'home'
        self.config = self.w / 'config.xml'
        config_xml.write(self.config, cfg)
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
                             cwd=str(self.w), stdout=subprocess.DEVNULL, stderr=t2.gate_app.window_log(self.err))
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
        time.sleep(1.5)
        tabs = [t['tab_id'] for t in self.o.call(1, 'list_tabs')['tabs']]
        self.tab = tabs[0]
        for extra in tabs[1:]:
            self.o.call(1, 'close_tab', tab_id=extra)
        until(lambda: len(self.o.call(1, 'list_tabs')['tabs']) == 1, 'one pane after closing the second')
        time.sleep(1)
        self.t0 = time.time()
        self.x('windowfocus', '--sync', self.wid)
        self.x('mousemove', '--window', self.wid, '700', '450', 'click', '1')
        self.x('mousemove', '--window', self.wid, '120', '600')
        time.sleep(.5)

    def shot(self, name):
        path = self.w / f'{name}.png'
        subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-f', 'x11grab', '-draw_mouse', '0', '-video_size',
                        f'{W}x{H}', '-i', self.env['DISPLAY'] + '+0,0', '-frames:v', '1', str(path)], env=self.env,
                       check=True, stdin=subprocess.DEVNULL)
        from PIL import Image
        return Image.open(path).convert('RGB')

    def info(self, tab=None):
        return self.o.call(3, 'terminal_info', tab_id=tab or self.tab)

    def workspace(self):
        return self.o.call(3, 'workspace_get')

    def terminals(self):
        return [t['tab_id'] for t in self.o.call(1, 'list_tabs')['tabs']]

    def saved(self):
        return config_xml.load(self.config)

    def click(self, x, y, button=1):
        self.x('mousemove', '--window', self.wid, str(int(x)), str(int(y)), 'click', str(button))
        time.sleep(.4)

    def away(self):
        self.x('mousemove', '--window', self.wid, '120', '600')
        time.sleep(.3)

    def alive(self):
        return self.p.poll() is None


def ocr_words(im, box, scale=3, psm=11):
    """Words in box as (text, x0, y0, x1, y1) in window pixels, lowercased."""
    from PIL import Image, ImageOps
    crop = im.crop(box).convert('L')
    if sum(crop.getdata()) / max(1, crop.width * crop.height) < 128:
        crop = ImageOps.invert(crop)
    crop = crop.resize((crop.width * scale, crop.height * scale), Image.LANCZOS)
    path = ROOT / 'ocr.png'
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


def ocr_text(im, box, scale=3, psm=11):
    return ' '.join(w[0] for w in ocr_words(im, box, scale, psm))


def find_word(words, pred, pick=min):
    hits = [w for w in words if pred(w[0])]
    return pick(hits, key=lambda w: w[1]) if hits else None


def centre(w):
    return (w[1] + w[3]) / 2, (w[2] + w[4]) / 2


def marks(im, y0, y1, x0, x1, bg=SIDEBAR):
    """Horizontal clusters (xa, xb) of columns holding a pixel more than 40 away from bg."""
    px = im.load()
    cols = [x for x in range(x0, x1) if any(diff(px[x, y], bg) > 40 for y in range(y0, y1))]
    out = []
    for x in cols:
        if out and x - out[-1][1] <= 3:
            out[-1][1] = x
        else:
            out.append([x, x])
    return [tuple(c) for c in out]


def blobs(im, colour, box, tol=12):
    """Horizontal runs of `colour` inside box: list of (x0, x1, y)."""
    px = im.load()
    runs = []
    for y in range(box[1], box[3]):
        x = box[0]
        while x < box[2]:
            if near(px[x, y], colour, tol):
                a = x
                while x < box[2] and near(px[x, y], colour, tol):
                    x += 1
                runs.append((a, x - 1, y))
            x += 1
    return runs


def frames_between(win, a, b):
    return [t for t in win.frames() if a <= t <= b]


def idle(win, label, seconds=2.5):
    """0 frames from 1 s to 1 + seconds s; avoids a minute boundary of the status line's uptime label."""
    while (time.time() - win.t0) % 60 > 60 - seconds - 3:
        time.sleep(1)
    t = time.time()
    time.sleep(seconds + 1.3)
    late = frames_between(win, t + 1, t + 1 + seconds)
    require(not late, f'{label}: 0 frames while nothing changes (observed {len(late)} in {seconds} s)')


def screen_rows(win, tab=None):
    lines = win.o.call(4, 'screen', tab_id=tab or win.tab, history_offset=0, history_limit=1)['lines']
    return [l if isinstance(l, str) else l.get('text', '') for l in lines]


def osc_block(command, output, code, sleep=None):
    """Shell text that prints one OSC 133 block (as gate-menu K13); output is printed after `sleep` seconds."""
    head = f"printf '\\033]133;A\\007$ \\033]133;B\\007{command}\\033]133;C\\007\\r\\n'"
    pause = f'; sleep {sleep}' if sleep else ''
    tail = f"; printf '{output}\\033]133;D;{code}\\007'" if code is not None else ''
    return head + pause + tail


def measure(win):
    p = Pane(win)
    p.n = 50
    p.measure()
    win.type('clear\n')
    time.sleep(.6)
    return p


def cols(win, tab=None):
    return win.info(tab)['cols']


# ---------------------------------------------------------------- parse, defaults, legacy rails

STONE_SIDEBAR = (15, 13, 11)  # assets/themes/stone-dark.json sidebar #0f0d0b, the default dark theme
DEFAULT_WINDOW = {}


def bad_value(key, value):
    """P2: a window on BASE plus one bad value opens with the default settings and Settings shows the line."""
    import numpy as np
    cfg = dict(BASE, theme='dark', theme_dark='graphite-dark', **{key: value})
    line = 2 + list(cfg).index(key) + 1
    assert config_xml.dump(cfg).splitlines()[line - 1].lstrip().startswith(f'<{key}>')
    label = f'P2 {key}={json.dumps(value)}'
    if 'cols' not in DEFAULT_WINDOW:
        ref = Win('p2-defaults', {})
        try:
            DEFAULT_WINDOW['cols'] = cols(ref)
        finally:
            ref.close()
    try:
        win = Win(f'p2-{key}', cfg)
    except Fail as e:
        raise Fail(f'{label}: the window opens anyway ({e})')
    try:
        time.sleep(3)
        require(win.alive(), f'{label}: the window is still running 3 s after it opened')
        c = cols(win)
        require(c == DEFAULT_WINDOW['cols'], f'{label}: the defaults, {DEFAULT_WINDOW["cols"]} columns as on an empty '
                                             f'config.xml (observed {c})')
        win.away()
        before = win.shot(f'p2-{key}-closed')
        require(near(before.getpixel((700, 890)), STONE_SIDEBAR, 3),
                f'{label}: the defaults, status line shown in Stone dark {STONE_SIDEBAR} at (700,890): '
                f'{before.getpixel((700, 890))}')
        win.key('ctrl+comma')
        time.sleep(1.2)
        after = win.shot(f'p2-{key}-settings')
        changed = np.abs(np.asarray(after).astype(int) - np.asarray(before).astype(int)).max(axis=2) > 12
        rows, columns = np.where(changed.sum(axis=1) >= 50)[0], np.where(changed.sum(axis=0) >= 50)[0]
        require(len(rows) and len(columns), f'{label}: Ctrl+, opens Settings')
        box = (int(columns[0]), int(rows[0]), int(columns[-1]) + 1, int(rows[-1]) + 1)
        texts = [ocr_text(after, box, 3, psm) for psm in (11, 6, 4)]
        require(any(re.search(rf'line\s*:?\s*{line}\b', t) for t in texts),
                f'{label}: Settings shows the error at "line {line}" (read {texts[0][-200:]!r})')
        require(win.alive(), f'{label}: the window is still running')
    finally:
        win.close()


def parse_story():
    try:
        win = Win('p1', dict(BASE, theme='dark', theme_dark='graphite-dark', tab_style='vertical', tools_panel='off',
                             tools=['files'], command_input='classic', block_style='cards', status_line='hide'))
    except Fail as e:
        raise Fail(f'P1 a config with every Q1 key starts the window ({e})')
    try:
        c = cols(win)
        require(c in (124, 125), f'P1 vertical tabs, tools off: 124-125 columns (observed {c})')
    finally:
        win.close()
    print('ok: P1 every Q1 key starts the window')
    for key, value in (('tab_style', 'diagonal'), ('tools_panel', True), ('tools', ['files', 'bogus']),
                       ('command_input', 'fancy'), ('block_style', 'loud'), ('status_line', 1)):
        bad_value(key, value)
    print('ok: P2 bad values open the window with the defaults and show the error line in Settings')
    win = Win('p3', dict(BASE, theme='dark', theme_dark='graphite-dark'))
    try:
        c = cols(win)
        require(c in (124, 125), f'P3 defaults: tools panel on, 124-125 columns (observed {c})')
        im = win.shot('p3')
        require(near(im.getpixel((700, 890)), SIDEBAR, 3), f'P3 defaults: status line shown at (700,890): {im.getpixel((700, 890))}')
    finally:
        win.close()
    print('ok: P3 defaults')


def legacy_story():
    for name, cfg, want, label in (
            ('l1', {'rails': FAULT == 'rails'}, (151, 152), 'L1 rails false hides the tools panel'),
            ('l2', {'rails': False, 'tools_panel': 'on'}, (124, 125), 'L2 tools_panel wins over rails')):
        win = Win(name, dict(BASE, theme='dark', theme_dark='graphite-dark', **cfg))
        try:
            c = cols(win)
            require(c in want, f'{label}: {want[0]}-{want[1]} columns (observed {c})')
        finally:
            win.close()
    print('ok: L1 L2 rails')


# ---------------------------------------------------------------- the main window

def make_tree(home):
    for d in ('alpha/assets', 'alpha/crates', 'alpha/docs', 'bravo/kilo', 'bravo/lima', 'charlie'):
        (home / d).mkdir(parents=True, exist_ok=True)
    for f in ('alpha/notes.txt', 'alpha/Cargo.toml', 'alpha/README.md', 'bravo/mike.txt'):
        (home / f).write_text('x\n')


def geometry(win, p):
    x0, y0, cw, ch = p.geo
    require(abs(cw - 9) <= .5 and abs(ch - 26) <= .5, f'setup: cells are 9 x 26 px (observed {cw:.2f} x {ch:.2f})')
    info = win.info()
    require(abs(x0 - 258) <= 3, f'G1 grid origin x is 236 + 8 + 14 = 258 (observed {x0:.1f})')
    require(42 <= y0 <= 70, f'G1 grid origin y in 42..70, no 40 px pane header (observed {y0:.1f})')
    require(info['cols'] in (124, 125), f'G1 124-125 columns (observed {info["cols"]})')
    require(y0 + info['rows'] * ch <= 874 + 1, f'G1 the last row ends above the status line ({y0 + info["rows"] * ch:.0f})')
    im = win.shot('g2')
    for (x, y), want, what in (((900, 4), SIDEBAR, 'title bar'), ((900, 33), SIDEBAR, 'title bar'),
                               ((900, 45), SIDEBAR, 'title bar last row, no line under it'),
                               ((120, 450), SIDEBAR, 'tools panel'),
                               ((235, 450), SIDEBAR, 'tools panel last column, no divider'),
                               ((240, 450), SIDEBAR, 'gap after the tools panel in the window colour'), ((300, 450), BG, 'pane'),
                               ((1300, 450), BG, 'pane where the right column was'), ((700, 890), SIDEBAR, 'status line'),
                               ((700, 874), LINE_SIDE, 'status line top hairline')):
        got = im.getpixel((x, y))
        require(near(got, want), f'G2 {what} at ({x},{y}) is {want} (observed {got})')
    text = ocr_text(im, (0, 875, W, H), 4, 7)
    m = re.search(r'(\d+) tabs?\b', text)
    require('session 1' in text and m and m.group(1) == '1' and 'server up' in text and 'masking' in text,
            f'G3 status line reads "Session 1", "1 tab", "server up", "masking" (read {text!r})')
    print('ok: G1 G2 G3 geometry, colours, status line')


def row_top(p, r):
    return p.geo[1] + (r - 1) * p.geo[3]


def line_at(im, p, r, colour, tol):
    """y values within 3 px of row r's top edge where >= 90 % of the samples across the pane match colour."""
    x0, _, cw, _ = p.geo
    xs = range(int(x0 + 20 * cw), int(x0 + 60 * cw), 3)
    top = int(round(row_top(p, r)))
    hits = []
    for y in range(top - 3, top + 3):
        n = sum(near(im.getpixel((x, y)), colour, tol) for x in xs)
        if n >= .9 * len(xs):
            hits.append(y)
    return hits


def ink(im, p, r, c0, n):
    x0, _, cw, ch = p.geo
    y = int(row_top(p, r))
    xs = range(int(x0 + (c0 - 1) * cw), int(x0 + (c0 - 1 + n) * cw))
    return sum(1 for yy in range(y + 3, int(y + ch) - 2) for xx in xs if sum(im.getpixel((xx, yy))) > sum(BG) + 150)


def err_pixels(im, p, r):
    x0, _, cw, ch = p.geo
    y = int(row_top(p, r))
    return sum(1 for yy in range(y + 3, int(y + ch) - 2) for xx in range(int(x0 + 20 * cw), W - 2)
               if near(im.getpixel((xx, yy)), ERR, 40) and im.getpixel((xx, yy))[0] > im.getpixel((xx, yy))[1] + 40)


def blocks_story(win, p):
    x0, y0, cw, ch = p.geo
    if FAULT == 'idle':
        win.type('clear; ' + osc_block('idlejob', '', None, 20) + '\n')
        time.sleep(2)
    else:
        win.type('clear\n')
        time.sleep(1)
    idle(win, 'I1')
    start = time.time()
    code = 0 if FAULT == 'exit' else 3
    win.type('clear; ' + osc_block('echo quietok', 'quietok\\r\\n', 0) + '; '
             + osc_block('false quietbad', 'quietbad-out\\r\\n', code) + '; '
             + osc_block('echo quietok2', 'quietok2\\r\\n', 0, 2.3) + '\n')
    until(lambda: [l.strip() for l in screen_rows(win)][:6] == ['$ echo quietok', 'quietok', '$ false quietbad',
                                                                'quietbad-out', '$ echo quietok2', 'quietok2'],
          'setup: three blocks on rows 1-6', 15)
    time.sleep(1)
    win.away()
    im = win.shot('blocks')
    require(not line_at(im, p, 1, LINE_BG, 5) and not line_at(im, p, 1, FAIL_LINE, 8), 'B1 no hairline above the first block')
    red = line_at(im, p, 3, FAIL_LINE, 8)
    require(1 <= len(red) <= 2, f'B1 the failed block has a 1-2 px hairline {FAIL_LINE} (rows matching: {red})')
    grey = line_at(im, p, 5, LINE_BG, 5)
    require(1 <= len(grey) <= 2, f'B1 the next block has a 1-2 px hairline {LINE_BG} (rows matching: {grey})')
    bold, regular = ink(im, p, 1, 8, 7), ink(im, p, 2, 1, 7)
    require(regular > 0 and bold >= 1.3 * regular, f'B2 the command row is bold ({bold} vs {regular} ink pixels)')
    e1, e3, e5 = err_pixels(im, p, 1), err_pixels(im, p, 3), err_pixels(im, p, 5)
    require(e3 >= 8 and e1 == 0 and e5 == 0, f'B3 err-coloured text only on the failed block (rows 1/3/5: {e1}/{e3}/{e5})')
    band = (int(x0 + 20 * cw), int(row_top(p, 3)) + 2, W, int(row_top(p, 4)) - 1)
    text = ocr_text(im, band, 4, 7).replace(' ', '')
    require('exit3' in text, f'B3 the failed block reads "exit 3" (read {text!r})')
    stamps = {time.strftime('%H:%M', time.gmtime(start + d)) for d in (0, 5, 10, 15, 20)}

    def meta_ok(meta):
        dur = [float(v) for v in re.findall(r'(\d+\.\d) ?s\b', meta)]
        return 'alpha' in meta and any(s in meta for s in stamps) and any(2.0 <= v <= 3.5 for v in dur)
    # Faint 10-12 px meta text: tesseract misreads a glyph at one scale and not at another, so the line passes
    # when any of four readings has every part. The expected parts do not change.
    box = (int(x0 + 20 * cw), int(row_top(p, 4)) + 2, W, int(row_top(p, 6)) - 1)
    reads = [ocr_text(im, box, sc, psm) for sc, psm in ((4, 11), (4, 6), (6, 6), (3, 11))]
    require(any(meta_ok(m) for m in reads),
            f'B4 meta text reads alpha, {"/".join(sorted(stamps))} and 2.0-3.5 s (read {reads!r})')
    print('ok: I1 B1-B4 quiet blocks')
    hover_story(win, p, red[0] if red else int(row_top(p, 3)), grey[0])


def hover_bar(win, p, hair):
    x0, _, cw, _ = p.geo
    win.away()
    a = win.shot('hover-before')
    win.x('mousemove', '--window', win.wid, str(int(x0 + 40 * cw)), str(int(row_top(p, 6) + p.geo[3] / 2)))
    time.sleep(.6)
    b = win.shot('hover')
    pa, pb = a.load(), b.load()
    pts = [(x, y) for y in range(40, 870) for x in range(240, W) if diff(pa[x, y], pb[x, y]) >= 15]
    require(pts, 'H1 hovering a block shows a bar')
    box = (min(x for x, _ in pts), min(y for _, y in pts), max(x for x, _ in pts) + 1, max(y for _, y in pts) + 1)
    return b, box


def hover_story(win, p, red, hair):
    b, box = hover_bar(win, p, hair)
    require(box[3] - box[1] <= 40, f'H1 one bar, at most 40 px tall (changed box {box})')
    require(abs(box[1] - (hair + 6)) <= 3 and abs(box[2] - (W - 14)) <= 3,
            f'H1 bar 6 px below the hairline (y {hair}) and 14 px from the right edge (changed box {box})')
    words = ocr_words(b, (box[0] - 4, box[1] - 4, min(W, box[2] + 4), box[3] + 4), 4, 7)
    text = ' '.join(w[0] for w in words)
    for want in ('copy output', 'copy command', 'run again', 'ask agent'):
        require(want in text, f'H1 the bar reads "{want}" (read {text!r})')
    for word, want in (('output', 'quietok2'), ('command', 'echo quietok2')):
        gm.set_clip(win, 'stale', 'clipboard')
        b, box = hover_bar(win, p, hair)
        w = find_word(ocr_words(b, (box[0] - 4, box[1] - 4, min(W, box[2] + 4), box[3] + 4), 4, 7), lambda t: t == word)
        require(w, f'H1 "{word}" found on the bar')
        win.click(*centre(w))
        got = gm.primary(win, 'clipboard') or ''
        require(got.rstrip('\n') == want and got.count('\n') <= 1, f'H1 Copy {word} copies {want!r} (got {got!r})')
    before = sum(l.strip() == 'quietok2' for l in screen_rows(win))
    b, box = hover_bar(win, p, hair)
    w = find_word(ocr_words(b, (box[0] - 4, box[1] - 4, min(W, box[2] + 4), box[3] + 4), 4, 7), lambda t: t == 'again')
    require(w, 'H1 "again" found on the bar')
    win.click(*centre(w))
    until(lambda: sum(l.strip() == 'quietok2' for l in screen_rows(win)) > before, 'H1 Run again runs it once more', 8)
    win.away()
    print('ok: H1 hover bar')


def sticky_story(win, p):
    x0, y0, cw, ch = p.geo
    win.type('clear; ' + osc_block('longjob', '', None) + '; seq 1 150; sleep 14; '
             "printf '\\033]133;D;0\\007'\n")
    until(lambda: any(l.strip() == '150' for l in screen_rows(win)), 'setup: 150 lines of output', 10)
    time.sleep(1.2)
    win.away()
    strip = (240, 38, W, int(y0 + ch + 4))
    text = ocr_text(win.shot('sticky-1'), strip, 4, 7)
    m = re.search(r'running (\d+) ?s', text)
    require('longjob' in text and m, f'S1 the pane top reads "longjob" and "running N s" (read {text!r})')
    t = time.time()
    time.sleep(3)
    text2 = ocr_text(win.shot('sticky-2'), strip, 4, 7)
    m2 = re.search(r'running (\d+) ?s', text2)
    require(m2 and 2 <= int(m2.group(1)) - int(m.group(1)) <= 5, f'S1 the running time counts up ({text!r} -> {text2!r})')
    time.sleep(1.2)
    n = len(frames_between(win, t, t + 4))
    require(2 <= n <= 8, f'S1 a running block redraws about once a second (observed {n} frames in 4 s)')
    until(lambda: any(l.startswith('sh') and l.rstrip().endswith('$') for l in screen_rows(win)[-6:]), 'setup: longjob ends', 20)
    time.sleep(1.5)
    text = ocr_text(win.shot('sticky-3'), strip, 4, 7)
    require('running' not in text, f'S1 "running" is gone after the block ends (read {text!r})')
    idle(win, 'S1 after the block ends')
    print('ok: S1 sticky command line')


def tree_words(win, name):
    return ocr_words(win.shot(name), (0, 40, 234, 800), 3, 11)


def files_story(win):
    def names(words):
        return {w[0]: w for w in words}
    want_dirs, want_files = ('assets', 'crates', 'docs'), ('notes.txt', 'cargo.toml', 'readme.md')
    words = until(lambda: (lambda ws: ws if all(n in names(ws) for n in want_dirs + want_files + ('alpha',)) else None)(
        tree_words(win, 'files-alpha')), 'F1 the tools panel lists ~/alpha: alpha, assets, crates, docs, notes.txt, '
                                         'Cargo.toml, README.md', 8)
    n = names(words)
    require(max(n[d][2] for d in want_dirs) < min(n[f][2] for f in want_files), 'F1 every folder is listed above every file')
    win.type('cd ~/bravo\n')
    until(lambda: (lambda ws: 'kilo' in ws and 'mike.txt' in ws and 'crates' not in ws)(names(tree_words(win, 'files-bravo'))),
          'F1 after cd ~/bravo the panel lists kilo and mike.txt, not crates', 6)
    (win.home / 'bravo/november.txt').write_text('n\n')
    until(lambda: 'november.txt' in names(tree_words(win, 'files-new')), 'F1 a new file appears within 4 s', 5)
    idle(win, 'F1 after the file tree settles')
    print('ok: F1 files')


def title_words(win, name):
    im = win.shot(name)
    return im, ocr_words(im, (40, 0, 1090, 37), 4, 11)


def tabs_story(win):
    win.key('ctrl+shift+t')
    until(lambda: len(win.workspace()['groups']) == 2, 'setup: Ctrl+Shift+T adds a tab')
    win.type('cd ~/alpha\n')
    im, words = until(lambda: (lambda r: r if find_word(r[1], lambda t: t == 'alpha') and find_word(r[1], lambda t: t == 'bravo') else None)(
        title_words(win, 'tabs')), 'T1 the title bar reads "bravo" and "alpha"', 8)
    a, b = find_word(words, lambda t: t == 'alpha'), find_word(words, lambda t: t == 'bravo')
    fill = lambda w: im.getpixel((int(w[1]) - 4, int(centre(w)[1])))
    require(near(fill(a), TAB_ON, 3) and not near(fill(b), TAB_ON, 3),
            f'T1 the active tab has the 8 % fill {TAB_ON}, the other not ({fill(a)} / {fill(b)})')
    win.click(*centre(b))
    until(lambda: win.workspace()['active'] == 0, 'T1 a click on "bravo" makes it the active tab')
    win.away()
    im, words = title_words(win, 'tabs-plus')
    a = find_word(words, lambda t: t == 'alpha')
    require(a, 'T1 "alpha" still in the title bar')
    plus = [m for m in marks(im, 6, 32, int(a[3]) + 5, 1090) if m[0] > a[3] + 5]
    require(plus, 'T1 a mark right of the last tab')
    win.click((plus[0][0] + plus[0][1]) / 2, 19)
    until(lambda: len(win.workspace()['groups']) == 3, 'T1 a click on "+" adds a tab')
    win.click(*centre(a))
    until(lambda: win.workspace()['active'] == 1, 'T1 a click on "alpha" makes it active')
    gone = win.workspace()['groups'][1]['focus']
    win.away()
    im, words = title_words(win, 'tabs-close')
    a = find_word(words, lambda t: t == 'alpha')
    x = [m for m in marks(im, 6, 32, int(a[3]) + 3, int(a[3]) + 31) if m[0] > a[3] + 3]
    require(x, 'T1 a close mark right of the active tab')
    win.click((x[0][0] + x[0][1]) / 2, 19)
    until(lambda: len(win.workspace()['groups']) == 2 and gone not in win.terminals(), 'T1 the close mark closes the tab')
    print('ok: T1 tabs')
    win.away()
    im, words = title_words(win, 'dots-0')
    b = find_word(words, lambda t: t == 'bravo')
    win.click(*centre(b))
    until(lambda: win.workspace()['active'] == 0, 'setup: tab "bravo" active')
    win.type('clear; ' + osc_block('dotrun', 'done\\r\\n', 0, 6) + '\n')
    time.sleep(1.5)
    win.away()
    im, words = title_words(win, 'dots-run')
    b = find_word(words, lambda t: t == 'bravo')
    ok = [r for r in blobs(im, OK, (0, 0, 1090, 37)) if 4 <= r[1] - r[0] + 1 <= 8]
    require(ok and b and any(b[1] - 22 <= r[0] <= b[1] for r in ok), f'T2 a green dot left of the running tab ("bravo" {b}, runs {ok[:3]})')
    time.sleep(6)
    im = win.shot('dots-done')
    require(not blobs(im, OK, (0, 0, 1090, 37)), 'T2 no green dot after the block ends')
    r = subprocess.run([str(APP), 'agent', '--socket', str(win.sock), '--config', str(win.config), '--tab', 'Fake',
                        str(win.home / 'charlie')], env=win.env, capture_output=True, text=True, timeout=30)
    require(r.returncode == 0, f'setup: clanky agent --tab Fake starts ({r.stderr.strip()[:200]})')
    time.sleep(2)
    win.away()
    im = win.shot('dots-agent')
    amber = [r for r in blobs(im, AGENT, (0, 0, 1090, 37)) if 4 <= r[1] - r[0] + 1 <= 8]
    require(amber, 'T2 an amber dot on the agent tab')
    print('ok: T2 state dots')


def bar_story(win):
    win.away()
    im = win.shot('bar')
    xs = [x for x in range(900, W) if near(im.getpixel((x, 19)), BG, 2)]
    require(xs, 'T3 a card-coloured search box in the title bar')
    xl, xr = min(xs), max(xs)
    ys = [y for y in range(0, 46) if near(im.getpixel((xl + 2, y)), BG, 2)]
    width, height = xr - xl + 1, (max(ys) - min(ys) + 1 if ys else 0)
    require(313 <= width <= 319 and 24 <= height <= 28, f'T3 search box inside is 313-319 x 24-28 px (observed {width} x {height})')
    require('search' in ocr_text(im, (xl, 0, xr, 46), 4, 7), 'T3 the search box reads "search"')
    win.away()
    im = win.shot('bar-2')
    m = marks(im, 6, 32, 0, W)
    gear, toggle = m[-1], m[0]
    require(gear[0] > xr and toggle[1] < 60, f'T3 gear right of the search box and toggle at the left ({m[:2]} ... {m[-1]})')
    win.click((gear[0] + gear[1]) / 2, 19)
    time.sleep(.8)
    require('appearance' in ocr_text(win.shot('gear'), (300, 140, 1100, 760), 3, 11), 'T3 the gear opens Settings')
    win.key('Escape')
    time.sleep(.6)
    tab = win.workspace()['groups'][win.workspace()['active']]['focus']
    c0 = cols(win, tab)
    win.click((toggle[0] + toggle[1]) / 2, 19)
    until(lambda: cols(win, tab) - c0 in (26, 27), f'T3 the toggle hides the tools panel (+26-27 columns from {c0})', 4)
    win.click((toggle[0] + toggle[1]) / 2, 19)
    until(lambda: cols(win, tab) == c0, 'T3 the toggle shows it again', 4)
    win.key('ctrl+shift+b')
    until(lambda: cols(win, tab) - c0 in (26, 27), 'T3 Ctrl+Shift+B hides the tools panel', 4)
    win.key('ctrl+shift+b')
    until(lambda: cols(win, tab) == c0, 'T3 Ctrl+Shift+B shows it again', 4)
    n = len(win.terminals())
    win.click((xl + xr) / 2, 19)
    time.sleep(.5)
    win.type('Split right')
    win.key('Return')
    until(lambda: len(win.terminals()) == n + 1, 'T3 a click on the search box opens the palette ("Split right" adds a pane)')
    print('ok: T3 search, gear, toggle')


def footer_story(win):
    win.away()
    words = ocr_words(win.shot('footer'), (0, 760, 236, 874), 3, 11)
    rec, ag = find_word(words, lambda t: t == 'recent'), find_word(words, lambda t: t == 'agents')
    require(rec and ag and find_word(words, lambda t: t == 'files'), f'T4 footer reads Files, Recent, Agents ({[w[0] for w in words]})')
    win.click(*centre(rec))
    win.away()
    text = ocr_text(win.shot('recent'), (0, 40, 236, 760), 3, 11)
    require('command' in text and 'mike.txt' not in text, f'T4 Recent lists commands, not files (read {text[:120]!r})')
    win.click(*centre(ag))
    win.away()
    words = ocr_words(win.shot('agents'), (0, 40, 236, 760), 3, 11)
    require('access' in ' '.join(w[0] for w in words), 'T4 Agents shows agent access')
    off = find_word(words, lambda t: t == 'off', pick=lambda h, key: min(h, key=lambda w: w[2]))
    require(off, 'T4 an access row reading "off"')
    before = {t['tab_id']: t['allow_agent'] for t in win.o.call(1, 'list_tabs')['tabs']}
    win.click(*centre(off))
    time.sleep(.6)
    after = {t['tab_id']: t['allow_agent'] for t in win.o.call(1, 'list_tabs')['tabs']}
    flipped = [k for k in before if before[k] is False and after.get(k) is True]
    require(len(flipped) == 1 and all(after[k] == before[k] for k in before if k not in flipped),
            f'T4 a click on an "off" row gives exactly one pane agent access ({before} -> {after})')
    print('ok: T4 Recent and Agents')


def main_story():
    bindir = ROOT / 'bin'
    bindir.mkdir(exist_ok=True)
    (bindir / 'fake-agent').write_text('#!/bin/sh\nprintf "FAKE-AGENT\\n"; sleep 60\n')
    (bindir / 'fake-agent').chmod(0o755)
    os.environ['PATH'] = f'{bindir}:{os.environ["PATH"]}'
    win = Win('main', dict(BASE, theme='dark', theme_dark='graphite-dark', command_input='classic',
                           agents=[{'name': 'Fake', 'command': 'fake-agent'}]))
    try:
        make_tree(win.home)
        win.type('cd ~/alpha; clear\n')
        time.sleep(1)
        p = measure(win)
        if FAULT not in ('idle', 'exit'):
            geometry(win, p)
        blocks_story(win, p)
        if FAULT:
            return
        sticky_story(win, p)
        files_story(win)
        tabs_story(win)
        footer_story(win)
        bar_story(win)
    finally:
        win.close()


# ---------------------------------------------------------------- Settings > Appearance > Layout

def open_appearance(win):
    win.key('ctrl+comma')
    time.sleep(.8)
    win.click(*NAV_APPEARANCE)
    time.sleep(.6)


def settings_open(win):
    return 'appearance' in ocr_text(win.shot('open?'), (310, 150, 540, 400), 3, 11)


def settings_story():
    win = Win('settings', dict(BASE, theme='dark', theme_dark='graphite-dark', rails=True, opacity=1))
    try:
        (win.home / 'charlie').mkdir()
        win.type('cd ~/charlie; clear; ' + osc_block('vtlastcmd', 'vt-out\\r\\n', 0) + '\n')
        until(lambda: any('vt-out' in l for l in screen_rows(win)), 'setup: block vtlastcmd')
        p = measure(win)
        win.type('clear; ' + osc_block('vtlastcmd', 'vt-out\\r\\n', 0) + '\n')
        time.sleep(2)
        x0, y0, cw, ch = p.geo
        block = (240, int(y0) - 6, W, int(y0 + 3 * ch))
        win.away()
        quiet = win.shot('quiet').crop(block)
        c0, r0 = win.info()['cols'], win.info()['rows']
        open_appearance(win)
        im = win.shot('layout')
        words = ocr_words(im, (539, 200, 1065, 700), 3, 11)
        seg = lambda name: find_word(words, lambda t: t == name, pick=max)
        tools = find_word(words, lambda t: t == 'tools')
        command = find_word(words, lambda t: t == 'command')
        require(tools and command, f'E1 the Layout group shows "Tools panel" and "Command input" ({[w[0] for w in words][:40]})')
        in_tools = lambda t: lambda w: w[0] == t and tools[2] - 12 <= w[2] < command[2] - 12
        found = {n: seg(n) for n in ('horizontal', 'vertical', 'editor', 'classic', 'quiet', 'cards', 'show', 'hide')}
        found['on'] = max((w for w in words if in_tools('on')(w)), key=lambda w: w[1], default=None)
        found['off'] = max((w for w in words if in_tools('off')(w)), key=lambda w: w[1], default=None)
        found['recent'] = find_word([w for w in words if in_tools('recent')(w)], lambda t: True)
        missing = [k for k, v in found.items() if not v]
        require(not missing, f'E1 Layout segments and chips found on screen (missing {missing})')
        pid = win.p.pid

        def press(name, key, value, check, label):
            t = time.time()
            win.click(*centre(found[name]))
            until(lambda: (win.saved().get(key, DEFAULTS[key]) == value), f'E1 {label}: config.xml {key} = {value!r} '
                  f'(file: {json.dumps({k: win.saved().get(k) for k in (key, "rails")})})', 3)
            require(win.alive() and win.p.pid == pid, f'E1 {label}: same window process, no restart')
            if check:
                until(check, f'E1 {label}: applied in the window', 3)
            require(frames_between(win, t, t + 1.5), f'E1 {label}: the window redraws at once')

        DEFAULTS = dict(tab_style='horizontal', tools_panel='on', tools=['files', 'recent', 'agents'],
                        command_input='editor', block_style='quiet', status_line='show')
        press('vertical', 'tab_style', 'vertical', lambda: c0 - win.info()['cols'] in (26, 27), 'Vertical')
        im = win.shot('vertical')
        left = ocr_text(im, (0, 40, 234, 860), 3, 11)
        title = ocr_text(im, (40, 0, 1090, 37), 4, 11)
        require('charlie' in left and 'vtlastcmd' in left and 'charlie' not in title,
                f'E1 vertical tabs list "charlie" and "vtlastcmd" on the left, not in the title bar ({left[:80]!r} / {title!r})')
        press('horizontal', 'tab_style', 'horizontal', lambda: win.info()['cols'] == c0, 'Horizontal')
        press('off', 'tools_panel', 'off', lambda: win.info()['cols'] - c0 in (26, 27), 'Tools panel Off')
        require('rails' not in win.saved(), 'E1 Tools panel Off removes "rails" from config.xml')
        press('on', 'tools_panel', 'on', lambda: win.info()['cols'] == c0, 'Tools panel On')
        require('rails' not in win.saved(), 'E1 Tools panel On leaves no "rails"')
        if FAULT != 'tools':
            win.click(*centre(found['recent']))
        until(lambda: win.saved().get('tools') == ['files', 'agents'], f'E1 chip Recent: tools ["files","agents"] '
              f'(file: {win.saved().get("tools")})', 3)
        foot = ocr_text(win.shot('tools-two'), (0, 760, 236, 874), 3, 11)
        require('recent' not in foot and 'files' in foot, f'E1 the footer loses Recent (read {foot!r})')
        press('recent', 'tools', ['files', 'recent', 'agents'], None, 'chip Recent again')
        press('classic', 'command_input', 'classic', None, 'Classic')
        press('editor', 'command_input', 'editor', None, 'Editor')
        press('cards', 'block_style', 'cards', lambda: win.shot('cards').crop(block).tobytes() != quiet.tobytes(), 'Cards')
        press('quiet', 'block_style', 'quiet', lambda: win.shot('quiet-2').crop(block).tobytes() == quiet.tobytes(), 'Quiet')
        press('hide', 'status_line', 'hide', lambda: win.info()['rows'] == r0 + 1, 'Hide')
        require(near(win.shot('hidden').getpixel((700, 890)), BG), 'E1 with the status line hidden (700,890) is the pane colour')
        press('show', 'status_line', 'show', lambda: win.info()['rows'] == r0, 'Show')
        idle(win, 'E2 after the last Settings click', 2)
        print('ok: E1 E2 Layout settings')
    finally:
        win.close()


# ---------------------------------------------------------------- Settings > Appearance > Theme

def scroll_to(win, word, notches=30):
    for _ in range(notches):
        words = ocr_words(win.shot('scroll'), (539, 200, 1065, 700), 3, 11)
        w = find_word(words, lambda t: t == word)
        if w and w[2] < 600:
            return w, words
        win.x('mousemove', '--window', win.wid, '800', '450', 'click', '5')
        time.sleep(.3)
    raise Fail(f'"{word}" found on the Appearance page')


def dots_ok(im, word, colours, chip, right):
    """Four dots along the word's row, right of it, in the given colour order."""
    y = int(centre(word)[1])
    found = []
    for c in colours:
        runs = [r for yy in (y - 1, y, y + 1) for r in blobs(im, c, (int(word[3]), yy, right, yy + 1), 6)
                if 6 <= r[1] - r[0] + 1 <= 12]
        if not runs:
            return f'no {c} dot'
        found.append(min((r[0] + r[1]) / 2 for r in runs))
    if found != sorted(found) or any(not 12 <= b - a <= 16 for a, b in zip(found, found[1:])):
        return f'dot centres {found}'
    mid = im.getpixel((int((found[0] + found[1]) / 2), y))
    if not near(mid, chip, 4):
        return f'chip colour {mid} between the first dots'
    return None


def theme_story():
    win = Win('theme', dict(BASE, theme='dark', theme_dark='stone-dark', theme_light='paper-light'))
    try:
        pal = win.home / '.local/share/org.gnome.Ptyxis/palettes'
        pal.mkdir(parents=True)
        (pal / 'harbor-quiet.palette').write_text(HARBOR_FILE)
        stone = (20, 18, 16)
        require(near(win.shot('k0').getpixel((1300, 450)), stone, 2), 'setup: Stone dark pane background')
        open_appearance(win)
        scroll_to(win, 'follow')
        before = win.config.read_bytes()
        px0 = win.shot('k1-0').getpixel((1300, 450))
        win.key('Down')
        time.sleep(.6)
        px1 = win.shot('k1-1').getpixel((1300, 450))
        require(px1 != px0 and win.config.read_bytes() == before,
                f'K1 Down previews the next theme ({px0} -> {px1}) without writing config.xml')
        win.key('Return' if FAULT == 'preview' else 'Escape')
        time.sleep(.6)
        px2 = win.shot('k1-2').getpixel((1300, 450))
        require(near(px2, px0, 1) and win.config.read_bytes() == before,
                f'K1 Escape puts the saved theme back ({px2} vs {px0}) and config.xml is unchanged')
        if not settings_open(win):
            open_appearance(win)
            scroll_to(win, 'follow')
        win.key('Down')
        time.sleep(.4)
        win.key('Return')
        until(lambda: win.saved().get('theme_dark') == 'paper-dark', f'K1 Down, Enter saves paper-dark ({win.saved()})', 3)
        if settings_open(win):
            win.key('Escape')
            time.sleep(.5)
        if settings_open(win):
            win.key('Escape')
            time.sleep(.5)
        px = win.shot('k1-paper').getpixel((1300, 450))
        require(near(px, (31, 28, 23), 2), f'K1 the window shows Paper dark (31,28,23) (observed {px})')
        print('ok: K1 preview')
        open_appearance(win)
        follow, words = scroll_to(win, 'follow')
        on = max((w for w in words if w[0] == 'on' and abs(centre(w)[1] - centre(follow)[1]) <= 24 and w[1] > follow[3]),
                 key=lambda w: w[1], default=None)
        require(on, 'K2 the Follow the system row has an "On" segment')
        win.click(*centre(on))
        until(lambda: win.saved().get('theme') == 'system', f'K2 Follow On writes theme "system" ({win.saved()})', 3)
        s = win.saved()
        require(s.get('theme_dark') == 'paper-dark' and s.get('theme_light') == 'paper-light',
                f'K2 Follow On keeps theme_dark and theme_light ({s})')
        themes = {f.stem: json.loads(f.read_text()) for f in (REPO / 'assets/themes').glob('*.json')}
        order = (1, 2, 3, 4) if FAULT == 'dots' else (1, 2, 4, 3)
        want = {}
        for col, rows in (('dark', DARK_ROWS), ('light', LIGHT_ROWS)):
            for word, tid in rows:
                t = themes[tid]
                want[(word, col)] = ([rgb(t['ansi'][i]) for i in order], rgb(t['colors']['background']))
        want[('harbor', None)] = ([rgb(HARBOR['ansi'][i]) for i in order], rgb(HARBOR['background']))
        done, problems, heads = set(), {}, set()
        for _ in range(30):
            im = win.shot('picker')
            # psm 11 skips a row framed by the chosen-row ring; psm 6 reads it. Use the words of both.
            words = ocr_words(im, (539, 200, 1065, 700), 3, 11) + ocr_words(im, (539, 200, 1065, 700), 3, 6)
            text = ' '.join(w[0] for w in words)
            heads |= {h for h in ('dark mode', 'light mode') if h in text}
            for w in words:
                col = 'dark' if centre(w)[0] < 800 else 'light'
                key = (w[0][:5] if w[0].startswith('frapp') else w[0], None if w[0] == 'harbor' else col)
                if key in want and key not in done and 205 < centre(w)[1] < 690:
                    right = 800 if col == 'dark' and key[1] else 1061
                    problem = dots_ok(im, w, *want[key], right)
                    if problem:
                        problems[key] = problem
                    else:
                        done.add(key)
                        problems.pop(key, None)
            if len(done) == len(want) or 'escape' in text:
                break
            win.x('mousemove', '--window', win.wid, '800', '450', 'click', '5')
            time.sleep(.3)
        require(heads == {'dark mode', 'light mode'}, f'K2 two columns "In dark mode" and "In light mode" (read {heads})')
        missing = [k for k in want if k not in done]
        require(not missing, f'K3 colour dots on every row: {[(k, problems.get(k, "row not found")) for k in missing][:4]}')
        print('ok: K2 K3 follow the system, colour dots')
        for _ in range(30):
            harbor = find_word(ocr_words(win.shot('harbor'), (539, 200, 1065, 700), 3, 11), lambda t: t == 'harbor')
            if harbor and 210 < centre(harbor)[1] < 690:
                break
            win.x('mousemove', '--window', win.wid, '800', '450', 'click', '4')
            time.sleep(.3)
        require(harbor, 'K4 Harbor Quiet row on screen')
        win.click(*centre(harbor))
        until(lambda: win.saved().get('terminal_colors') == HARBOR,
              f'K4 a click on Harbor Quiet writes its terminal_colors ({win.saved().get("terminal_colors")})', 3)
        s = win.saved()
        require(s.get('theme') == 'system' and s.get('theme_dark') == 'paper-dark', f'K4 theme keys unchanged ({s})')
        for _ in range(2):
            if settings_open(win):
                win.key('Escape')
                time.sleep(.5)
        px = win.shot('harbor-pane').getpixel((1300, 450))
        require(near(px, rgb(HARBOR['background']), 2), f'K4 the pane is #102030 (observed {px})')
        print('ok: K4 Ptyxis palette')
    finally:
        win.close()


def main():
    for tool in ('Xvfb', 'xdotool', 'xclip', 'ffmpeg', 'tesseract'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    stories = {None: [parse_story, legacy_story, main_story, settings_story, theme_story],
               'rails': [legacy_story], 'dots': [theme_story], 'preview': [theme_story], 'tools': [settings_story],
               'exit': [main_story], 'idle': [main_story]}
    require(FAULT in stories, f'known fault {FAULT}')
    for story in stories[FAULT]:
        story()
    if FAULT is None:
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: Q1 quiet look')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: Q1 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: Q1 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
