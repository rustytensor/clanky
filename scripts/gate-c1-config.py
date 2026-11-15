#!/usr/bin/env python3
"""C1 gate: every setting in Settings, and an XML config file (milestone C1, Marc 2026-11-14: "We need all the options
in the config file exposed in the control panel. The user shouldn't have to manually edit a config file. Also, we
should use XML for the config file." and "Good feel and good looks are more important than having a million
features.").

Usage: gate-c1-config.py [--fault typo|colour|agent|idle|malformed|migrate|legacy]
A fault changes the run's input on purpose; a correct build must then FAIL.

Harness: the Q1 gate's window (scripts/gate-q1-look.py Win, itself the T2 harness: a private Xvfb display from :100
up, 1400 x 900 px at scale 1, software Vulkan, TZ=UTC, SHELL=/bin/sh, screenshots without the pointer, the second
startup pane closed), which writes its config with scripts/clanky_config_xml.py and starts Clanky with
`--config DIR/config.xml`. Clicks and keys are real X input (xdotool). Text on screen is read with tesseract, and
always more than one way (as the Q1 gate's B4 does): plain OCR at scale/psm 3/11, 4/6 and 4/11 and a
contrast-stretched reading for faint text; a label or value is found when one reading has it. The expected words never
change between readings. config.xml is read two ways: Python's xml.etree.ElementTree (an independent XML 1.0 parser,
never Clanky) and a regular expression on the raw text for every scalar element.

Interface fixed by this gate.
  File. ~/.config/clanky/config.xml ($XDG_CONFIG_HOME/clanky/config.xml); `clanky --config PATH` reads PATH as XML.
  One root element <clanky version="1">, one child element per setting, named exactly as the key in docs/config.md,
  the value as text: booleans true | false, integers in decimal, numbers in decimal without an exponent and without
  float noise (0.85, 1.2, 1.5, 18: the shortest decimal that reads back as the stored value), strings as text.
  Structured values:
    tools            <tools><tool>files</tool><tool>agents</tool></tools>
    colors           <colors><accent>#3a7bd5</accent></colors>            (one element per colour token)
    terminal_colors  <terminal_colors><background>#rrggbb</background><foreground>..</foreground><cursor>..</cursor>
                     <ansi><color>#rrggbb</color> sixteen times, 0..15 in order</ansi></terminal_colors>
    agents           <agents><agent><name>Claude Code</name><command>claude</command></agent> ...</agents>
  Whitespace between elements, comments and an XML declaration are allowed. A Settings write of a default value may
  remove its element. The legacy keys font_size_pt and rails are still read from config.xml with their old meaning
  (older gates and hand-written files use them) but Settings never shows them, and migration writes their modern
  form instead. layout.json stays JSON.
  Migration. On a start with $XDG_CONFIG_HOME/clanky/config.json and no config.xml beside it, Clanky writes
  config.xml with the same values and renames config.json to config.json.migrated (byte for byte the old file).
  font_size_pt P becomes font_size P x 4 / 3 (docs/config.md: "12 points equals 16 logical pixels"), rails false/true
  becomes tools_panel off/on, and when the modern key is also present it wins (docs/config.md). With both files
  present nothing is migrated and config.json is left alone. --config never migrates.
  Bad files. A malformed config.xml or a bad value never stops the window: it opens, keeps the last good values
  (defaults at a cold start), Settings shows the error with its line ("line N", N the 1-based line of the fault), and
  the file is not rewritten until the user changes something in Settings; that change writes a well-formed file.
  Settings. Ctrl+, opens it; a nav column on its left with the groups, top to bottom, General, Appearance, Terminal,
  Editor, Agents, History, Keys (Appearance stays the second row, as the Q1 gate fixes); a click on a group shows its
  page; the wheel over a page scrolls it. Each setting is a row whose label is exactly the text below; its control
  sits on the label's line or is centred on the label and its description, at most 40 px below the label's top, at
  the right. Controls:
    switch   booleans: a click on it flips the value (an On | Off segment pair is accepted as well);
    segments labelled words; a click on one saves it;
    field    shows the stored value as written in config.xml; a click on the value focuses the field, Ctrl+A
             selects its text, typing replaces it, Enter saves (Escape cancels);
    swatch   a colour square (10-64 px, filled with the colour it stands for); a click opens its picker with a hex
             field that has the focus: Ctrl+A, "#rrggbb", Enter saves and closes the picker. Each swatch row of
             Window colours also has a "Reset" button that removes that token's override.
  Every change is written to config.xml within 3 s and applied at once in the same window process (no restart);
  a row whose setting takes effect later says so ("applies to new panes" / "after restart").
    General     Post-AI mode (switch, post_ai), Restore layout (switch, restore_layout), Robot greeting (switch,
                startup_animation), Animations (switch, animations), Notify after (field, notify_after_seconds),
                Columns (field, columns), Rows (field, rows)
    Appearance  Follow the system On | Off and the theme lists of the Q1 gate (theme, theme_dark, theme_light), the
                Layout group of the Q1 gate (segments Horizontal | Vertical, Tools panel On | Off, chips Files Recent
                Agents, Editor | Classic, Quiet | Cards, Show | Hide), Font size (field, font_size), Terminal font
                (field, font_family), Interface font (field, ui_font_family), Cell width (field, cell_width_scale),
                Line height (field, cell_height_scale), Opacity (field, opacity), Window colours (one swatch per
                colour token, colors)
    Terminal    Cursor blink (segments System | On | Off, cursor_blink), Blink cycle (field, cursor_blink_time_ms),
                Stop blinking after (field, cursor_blink_timeout_s), Clipboard writes (switch, osc52_write),
                Clipboard reads (switch, osc52_read), Colours: swatches Background, Foreground, Cursor and ANSI 0-15
                (terminal_colors)
    Editor      Dictionary (field, compose_dictionary), Dictionary folder (field, compose_dictionary_dir), Enter
                after send (switch, compose_send_enter)
    Agents      New agent panes (segments Read + run | Off, agent_pane_access); the agent list (agents): one row per
                agent with its name and command and a "Remove" button, and an "Add agent" button that adds a row and
                focuses its name field (type the name, Tab, the command, Enter saves)
    History     Save history (switch, history), Keep for (field, history_days), Output per command (field,
                history_output_kib)
    Keys        the shortcuts, read only
  That is all 37 keys of docs/config.md (39 rows less the legacy font_size_pt and rails).

Sources. [D] docs/config.md at main 6b12597: the keys, types, defaults and ranges (font_size 8-32, opacity 0.5-1.0,
cell scales 1.0-2.0, cursor_blink_time_ms 100-10000, history_days 0-3650, history_output_kib 1-65536, agents up to
32 {name, command}; defaults post_ai false, restore_layout true, startup_animation true, animations true,
notify_after_seconds 10, columns 80, rows 24, osc52_write true, osc52_read false, compose_dictionary en_US,
compose_dictionary_dir /usr/share/hunspell, compose_send_enter false, agents Claude Code/claude, Codex/codex, Pi/pi,
agent_pane_access read+run, history true, history_days 90, history_output_kib 256, cursor_blink system,
cursor_blink_time_ms 1200, cursor_blink_timeout_s 10). [Q1] scripts/gate-q1-look.py: the Layout and theme picker
interface and its effects (Vertical takes 26-27 columns, Tools panel Off gives them back, Hide gives a row).
[T] assets/themes/*.json. [X] XML 1.0 as Python's ElementTree reads it. [J] Python's json module reading the
migrated config.json. [V] xterm SGR: 40-47 paint ANSI 0-7 backgrounds, 100-107 ANSI 8-15. [F] fc-list on this
machine: DejaVu Sans Mono and IBM Plex Sans are installed.

The window's starting config (BASE), written by the gate: startup_animation false, animations false, cursor_blink
"off", theme "dark", theme_dark graphite-dark, theme_light graphite-light, font_family "IBM Plex Mono", font_size 15,
colors {accent #b8327a, err #d2462c} and terminal_colors OLD (all 19 colours distinct, none grey):
  OLD background #1b2a3a foreground #d9c8a9 cursor #f2a541
      ansi #2e1f4d #c23b3b #3bb273 #d9a21b #3b6fd9 #b03bd9 #2bb3b3 #c8b88a #5a4a7a #ff6b6b #6bff9e #ffd76b #6b9eff
           #e06bff #6bfff0 #fff3c4
  NEW background #0f1d14 foreground #e8e2f0 cursor #7fd1ff
      ansi #401020 #e0505a #50c878 #e6b450 #4f8fe6 #c060e0 #40c0c0 #d0d8b0 #604860 #ff8080 #90f090 #f8e080 #80b0ff
           #f0a0ff #a0f0f0 #fafae0

Expected results, fixed before the work.
  S0 a window starts on a config.xml holding BASE; Settings opens and its nav reads the seven groups in the order
     above.
  S1 every key, set through the Settings window in one window process. After each action config.xml holds the value
     below within 3 s (ElementTree value and, for scalars, the raw text <key>TEXT</key> exactly once), and the effect
     shows where it is visible:
     General   restore_layout false; startup_animation: one click (true or removed), a second click false; animations
               the same; notify_after_seconds 25; columns 100; rows 30 (the last two apply to new windows).
     Terminal  cursor_blink "on", then "off"; cursor_blink_time_ms 800; cursor_blink_timeout_s 5; osc52_write false;
               osc52_read true; each of the 19 swatches found by its OLD colour and set to its NEW colour, one at a
               time, terminal_colors[...] equal to NEW after each. Effect with Settings closed: after
               printf of two spaces in SGR 40..47 and 100..107 [V] every NEW ansi colour covers a cell-sized block
               (>= 8 x 10 px, within 2) in the window, and an empty pane pixel is NEW background (within 2).
     Editor    compose_dictionary "de_DE"; compose_dictionary_dir HOME/dicts (a folder the gate makes);
               compose_send_enter true.
     Agents    agent_pane_access "off"; Add agent "Fake Tool" / "fake-tool --x" gives agents Claude Code/claude,
               Codex/codex, Pi/pi, Fake Tool/fake-tool --x; Remove on the Pi row gives Claude Code/claude,
               Codex/codex, Fake Tool/fake-tool --x. Effect: with Settings closed, Ctrl+Shift+G (the agent picker)
               reads "fake" and no word "pi".
     History   history false; history_days 30; history_output_kib 512.
     Keys      the page reads "ctrl" and "tab" (the Ctrl+Shift+T row of docs/shortcuts.md).
     Appearance  Follow the system On: theme "system"; "Mocha" in the dark list: theme_dark "catppuccin-mocha" and the
               title bar pixel (900,4) changes (by > 6 in a channel); "Paper" in the light list: theme_light
               "paper-light"; Follow Off: theme "dark" (no desktop style under GSETTINGS_BACKEND=memory: dark [D]).
               Vertical: tab_style "vertical", >= 10 fewer columns [Q1]; Tools panel Off: tools_panel "off", >= 10
               more columns; chip Recent: tools ["files","agents"]; Classic: command_input "classic"; Cards:
               block_style "cards"; Hide: status_line "hide", >= 1 more row. Font size 15 -> 18: font_size 18,
               columns at most 0.9 x before; Terminal font -> "DejaVu Sans Mono" [F]: font_family, the strip
               y 40..140 of the pane (above Settings) changes; Interface font -> "IBM Plex Sans" [F]: ui_font_family,
               the nav column's pixels change; Cell width 1 -> 1.2: at most 0.9 x the columns; Line height 1.25 -> 1.5
               (R1 2026-11-14 made 1.25 the default):
               at most 0.9 x the rows; Window colours: the accent swatch (#b8327a) set to #3a7bd5 gives colors
               {accent #3a7bd5, err #d2462c} and a #3a7bd5 swatch on screen; Reset on the err swatch's row gives
               colors {accent #3a7bd5}; Opacity 1 -> 0.85 (last, so no pixel check above sees the alpha).
     General   post_ai true (last: Post-AI mode turns agents off).
     The window process is the same one from S0 to here.
  X1 the final config.xml: root <clanky version="1">, exactly the 37 keys, each equal to EXPECT (the values above),
     no font_size_pt and no rails; tools children all <tool>, agents children <agent> with exactly <name> and
     <command>, terminal_colors children background, foreground, cursor and <ansi> with 16 <color>.
  I1 0 frames from 1 s to 3.5 s after the last change with Settings open; I2 the same after Settings closes.
  X2 a new window started on that final file opens, its Settings shows no "line N" error, and the file is byte for
     byte the same 3 s later.
  B1 cold start on a malformed file (line 4 "<font_size>16</font_sizee>"): the window opens; Settings reads
     "line 4"; the file is unchanged after 3 s; Restore layout clicked: config.xml is well-formed [X] with
     restore_layout false.
  B2 cold start with a bad value (line 3 "<opacity>0.3</opacity>", below 0.5 [D]): the window opens, Settings reads
     "line 3" and "opacity", the file is unchanged after 3 s.
  B3 live: a window on {BASE, tab_style vertical}; the file is replaced by one malformed at line 5: within 3 s
     Settings reads "line 5", the columns stay the same (last good values kept), the file is unchanged 3 s later;
     then Restore layout clicked: config.xml well-formed with tab_style "vertical" and restore_layout false.
  M1 a copy of Marc's own config.json, or of his config.json.migrated when his window has already migrated it (the
     same bytes, docs/config.md) (read only; the copy lives in the gate's folder and is deleted with it) as
     $XDG_CONFIG_HOME/clanky/config.json, no config.xml, no --config: within 5 s config.xml exists, config.json is
     gone, config.json.migrated equals the copy byte for byte, and config.xml has exactly the copy's keys (an empty
     colors object may be left out), each value equal to what Python's json reads [J] (numbers within 1e-6, colours
     compared lowercase); Settings shows no "line N" error.
  M2 a legacy config.json {"font_size_pt": 12, "rails": false, "columns": 90, "theme": "light"} migrates to exactly
     font_size 16, tools_panel "off", columns 90, theme "light" (no font_size_pt, no rails).
  M3 a config.json holding EXPECT (all 37 keys) migrates to a config.xml equal to EXPECT, key for key.
  M4 config.json {"tab_style": "horizontal"} beside config.xml {tab_style vertical}: both files are byte for byte
     unchanged 3 s after the window opens, and there is no config.json.migrated.
  D1 docs/config.md has an ```xml example that ElementTree parses: root <clanky version="1"> with an element for each
     of the 37 keys, none for font_size_pt or rails.
  D2 every line of docs/*.md (the design-*.md history files aside) that names config.json also says "migrat", and
     docs/config.md names config.xml and config.json.migrated.
Planted faults: typo (Notify after typed as 26 while 25 is expected), colour (the ANSI 5 swatch is not clicked),
agent (Remove is clicked on the Codex row instead of Pi), idle (a shell loop prints a line every 0.4 s during I1),
malformed (B3 writes a well-formed file instead of the broken one), migrate (M1's window is started with --config on
an empty config.xml, so no migration happens), legacy (M2's config.json has rails true).
"""
import importlib.util, json, os, re, shutil, subprocess, sys, time
from collections import deque
from difflib import SequenceMatcher
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_C1_WORK', '/srv/devops/build/clanky-c1-gate-run'))
os.environ['CLANKY_Q1_WORK'] = str(ROOT)
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
_argv = sys.argv
sys.argv = sys.argv[:1]  # the harness modules read their own --fault from argv; they get none
spec = importlib.util.spec_from_file_location('q1', Path(__file__).with_name('gate-q1-look.py'))
q1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q1)
sys.argv = _argv
config_xml = q1.config_xml
t2 = q1.t2
Fail, require, until = q1.Fail, q1.require, q1.until
REPO = Path(__file__).resolve().parents[1]
APP = q1.APP
W, H = 1400, 900
MARC_CONFIG = Path('/home/marcm/.config/clanky/config.json')
# Once Marc's own window has migrated, config.json is gone and config.json.migrated holds the same bytes
# (docs/config.md: "renames the old file to config.json.migrated, unchanged").
MARC_CONFIG_MIGRATED = Path('/home/marcm/.config/clanky/config.json.migrated')

OLD = {'background': '#1b2a3a', 'foreground': '#d9c8a9', 'cursor': '#f2a541',
       'ansi': ['#2e1f4d', '#c23b3b', '#3bb273', '#d9a21b', '#3b6fd9', '#b03bd9', '#2bb3b3', '#c8b88a',
                '#5a4a7a', '#ff6b6b', '#6bff9e', '#ffd76b', '#6b9eff', '#e06bff', '#6bfff0', '#fff3c4']}
NEW = {'background': '#0f1d14', 'foreground': '#e8e2f0', 'cursor': '#7fd1ff',
       'ansi': ['#401020', '#e0505a', '#50c878', '#e6b450', '#4f8fe6', '#c060e0', '#40c0c0', '#d0d8b0',
                '#604860', '#ff8080', '#90f090', '#f8e080', '#80b0ff', '#f0a0ff', '#a0f0f0', '#fafae0']}
ACCENT_OLD, ERR_OLD, ACCENT_NEW = '#b8327a', '#d2462c', '#3a7bd5'
BASE = dict(startup_animation=False, animations=False, cursor_blink='off', theme='dark', theme_dark='graphite-dark',
            theme_light='graphite-light', font_family='IBM Plex Mono', font_size=15,
            colors={'accent': ACCENT_OLD, 'err': ERR_OLD}, terminal_colors=OLD)
NAV = ['general', 'appearance', 'terminal', 'editor', 'agents', 'history', 'keys']
DEFAULT_AGENTS = [{'name': 'Claude Code', 'command': 'claude'}, {'name': 'Codex', 'command': 'codex'},
                  {'name': 'Pi', 'command': 'pi'}]
FAKE = {'name': 'Fake Tool', 'command': 'fake-tool --x'}


def expect_all(home):
    """EXPECT: the 37 keys after S1."""
    return dict(
        post_ai=True, restore_layout=False, startup_animation=False, animations=False, notify_after_seconds=25,
        columns=100, rows=30,
        theme='dark', theme_dark='catppuccin-mocha', theme_light='paper-light', tab_style='vertical',
        tools_panel='off', tools=['files', 'agents'], command_input='classic', block_style='cards',
        status_line='hide', font_size=18, font_family='DejaVu Sans Mono', ui_font_family='IBM Plex Sans',
        opacity=0.85, cell_width_scale=1.2, cell_height_scale=1.5, colors={'accent': ACCENT_NEW},
        cursor_blink='off', cursor_blink_time_ms=800, cursor_blink_timeout_s=5, osc52_write=False, osc52_read=True,
        terminal_colors=NEW,
        compose_dictionary='de_DE', compose_dictionary_dir=str(home / 'dicts'), compose_send_enter=True,
        agent_pane_access='off', agents=[DEFAULT_AGENTS[0], DEFAULT_AGENTS[1], FAKE],
        history=False, history_days=30, history_output_kib=512)


KEYS = sorted(expect_all(Path('/h')))
assert len(KEYS) == 37


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def diff(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


# ---------------------------------------------------------------- the config file, read two ways

def text_of(v):
    """The element text a scalar must have."""
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return repr(v)
    from xml.sax.saxutils import escape
    return escape(v)


def same(a, b, tol=1e-9):
    if isinstance(b, bool) or isinstance(a, bool):
        return a is b
    if isinstance(b, (int, float)) and isinstance(a, (int, float)):
        return abs(a - b) <= tol * max(1, abs(b))
    if isinstance(b, str) and isinstance(a, str):
        return a == b if not b.startswith('#') else a.lower() == b.lower()
    if isinstance(b, list) and isinstance(a, list):
        return len(a) == len(b) and all(same(x, y, tol) for x, y in zip(a, b))
    if isinstance(b, dict) and isinstance(a, dict):
        return a.keys() == b.keys() and all(same(a[k], b[k], tol) for k in b)
    return False


def raw_ok(raw, key, value):
    if isinstance(value, (list, dict)):
        return True
    return len(re.findall(rf'<{key}>\s*{re.escape(text_of(value))}\s*</{key}>', raw)) == 1


def file_value(path, key):
    try:
        raw = Path(path).read_text()
        return config_xml.loads(raw).get(key, '<absent>'), raw
    except Exception:
        return '<unreadable>', ''


def short(v):
    s = json.dumps(v) if not isinstance(v, str) else v
    return s if len(s) < 120 else s[:117] + '...'


def expect(win, key, want, label, absent_ok=False, timeout=3):
    def ok():
        got, raw = file_value(win.config, key)
        if got == '<absent>':
            return absent_ok
        return same(got, want) and raw_ok(raw, key, want)
    try:
        until(ok, label, timeout)
    except Fail:
        got, raw = file_value(win.config, key)
        line = re.search(rf'<{key}>.*?</{key}>', raw, re.S)
        raise Fail(f'{label}: config.xml <{key}> = {short(want)}'
                   f'{" or absent" if absent_ok else ""} (file: {short(got)}; text {short(line.group(0)) if line else "-"})')
    require(win.alive() and win.p.pid == win.pid0, f'{label}: same window process, no restart')


# ---------------------------------------------------------------- the window

class C1Win(q1.Win):
    """The Q1 window; cfg is a dict (written as config.xml), a str (written as is) or None (no file). With
    explicit=False Clanky gets no --config and reads $XDG_CONFIG_HOME/clanky; prepare(w) runs before the start."""

    def __init__(self, name, cfg, explicit=True, prepare=None):
        self.cfg_in, self.explicit, self.prepare = cfg, explicit, prepare
        super().__init__(name, cfg)
        self.pid0 = self.p.pid
        self.frame = self.nav = self.content = None


def _start(self, cfg, extra_env):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('XDG_', 'CLANKY', 'WAYLAND', 'DBUS', 'GNOME', 'GSETTINGS'))}
    env.update(HOME=str(self.w / 'home'), XDG_CONFIG_HOME=str(self.w / 'config'), XDG_DATA_HOME=str(self.w / 'data'),
               XDG_STATE_HOME=str(self.w / 'state'), XDG_RUNTIME_DIR=str(self.w / 'runtime'), SHELL='/bin/sh',
               TZ='UTC', GSETTINGS_BACKEND='memory', WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1',
               LP_NUM_THREADS='8', VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json',
               CLANKY_FRAME_TRACE=str(self.w / 'frames.csv'))
    self.home = self.w / 'home'
    (self.home / 'dicts').mkdir()
    self.config = self.w / 'config.xml' if self.explicit else self.w / 'config/clanky/config.xml'
    self.config.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(cfg, dict):
        config_xml.write(self.config, cfg)
    elif isinstance(cfg, str):
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
    args = [str(APP), '--socket', str(self.sock)] + (['--config', str(self.config)] if self.explicit else [])
    p = subprocess.Popen(args, env=env, cwd=str(self.w), stdout=subprocess.DEVNULL, stderr=self.err.open('wb'))
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


C1Win._start = _start


def start(name, cfg, label, **kw):
    try:
        return C1Win(name, cfg, **kw)
    except Fail as e:
        raise Fail(f'{label} ({e})')


# ---------------------------------------------------------------- reading the screen more than one way

def faint_words(im, box, scale=3, sat=40):
    """OCR with every pixel's distance from the box's commonest grey stretched (40 levels = full ink)."""
    crop = im.crop(box).convert('L')
    hist = crop.histogram()
    bg = hist.index(max(hist))
    ink = crop.point(lambda v: 255 - min(255, abs(v - bg) * 255 // sat)).convert('RGB')
    return [(t, box[0] + a, box[1] + b, box[0] + c, box[1] + d)
            for t, a, b, c, d in q1.ocr_words(ink, (0, 0, ink.width, ink.height), scale, 11)]


def readings(im, box):
    """Several readings of the same pixels, produced lazily."""
    yield q1.ocr_words(im, box, 3, 11)
    yield q1.ocr_words(im, box, 4, 6)
    yield faint_words(im, box, 3)
    yield q1.ocr_words(im, box, 4, 11)


def norm(t):
    return re.sub(r'[^a-z0-9+#._/-]', '', t.lower())


def token_ok(read, want):
    a, b = norm(read), norm(want)
    if len(b) <= 3:
        return a == b
    return a == b or SequenceMatcher(None, a, b).ratio() >= .8


def find_phrase(words, phrase):
    """Box (x0, y0, x1, y1) of the words of phrase, in order on one line, or None."""
    toks = phrase.lower().split()
    ws = sorted(words, key=lambda w: (round(w[2] / 8), w[1]))
    for w in ws:
        if not token_ok(w[0], toks[0]):
            continue
        box, last = list(w[1:]), w
        for t in toks[1:]:
            nxt = [v for v in ws if abs((v[2] + v[4]) / 2 - (last[2] + last[4]) / 2) <= 7 and 0 <= v[1] - last[3] <= 40
                   and token_ok(v[0], t)]
            if not nxt:
                box = None
                break
            last = min(nxt, key=lambda v: v[1])
            box = [box[0], min(box[1], last[2]), last[3], max(box[3], last[4])]
        if box and not [v for v in ws if abs((v[2] + v[4]) / 2 - (box[1] + box[3]) / 2) <= 7
                        and 0 <= v[1] - box[2] <= 14]:
            return tuple(box)  # a label is whole: no word right after it ("Dictionary" is not "Dictionary folder")
    return None


def all_text(im, box):
    return [' '.join(w[0] for w in r) for r in readings(im, box)]


# ---------------------------------------------------------------- Settings

def settings_open(win):
    if not win.frame:
        return False
    nav = (win.frame[0], win.frame[1], win.content[0], win.frame[3])
    return any(sum(1 for n in NAV if any(token_ok(w[0], n) for w in r)) >= 4 for r in readings(win.shot('open?'), nav))


def open_settings(win, label):
    """Ctrl+, ; the frame is the box of pixels that changed; the nav is its left column."""
    import numpy as np
    if settings_open(win):
        return
    before = np.asarray(win.shot('closed')).astype(int)
    win.key('ctrl+comma')
    time.sleep(1)
    after = np.asarray(win.shot('settings')).astype(int)
    changed = np.abs(after - before).max(axis=2) > 12
    rows, cols = np.where(changed.sum(axis=1) >= 50)[0], np.where(changed.sum(axis=0) >= 50)[0]
    require(len(rows) and len(cols), f'{label}: Ctrl+, opens Settings')
    fx0, fy0, fx1, fy1 = int(cols[0]), int(rows[0]), int(cols[-1]), int(rows[-1])
    im = win.shot('settings')
    navbox = (fx0, fy0, fx0 + int(.34 * (fx1 - fx0)), fy1)
    found = None
    for words in readings(im, navbox):
        hits = {n: next((w for w in words if token_ok(w[0], n)), None) for n in NAV}
        if all(hits.values()):
            found = hits
            break
    require(found, f'{label}: the Settings nav reads {", ".join(n.title() for n in NAV)} '
                   f'(read {all_text(im, navbox)[0][:160]!r})')
    ys = [(found[n][2] + found[n][4]) / 2 for n in NAV]
    require(ys == sorted(ys), f'{label}: nav groups in the order {", ".join(n.title() for n in NAV)} ({ys})')
    win.frame, win.nav = (fx0, fy0, fx1, fy1), found
    right = max(w[3] for w in found.values())
    win.content = (int(right) + 16, fy0, fx1 - 4, fy1)


def close_settings(win):
    for _ in range(4):
        if not settings_open(win):
            return
        win.key('Escape')
        time.sleep(.5)
    raise Fail('Escape closes Settings')


def page(win, name):
    w = win.nav[name.lower()]
    win.click((w[1] + w[3]) / 2, (w[2] + w[4]) / 2)
    time.sleep(.7)


def wheel(win, button, n=1):
    cx, cy = (win.content[0] + win.content[2]) // 2, (win.content[1] + win.content[3]) // 2
    win.x('mousemove', '--window', win.wid, str(cx), str(cy), 'click', '--repeat', str(n), '--delay', '30', str(button))
    time.sleep(.4)


def seek(win, found, what, notches=30):
    """Scrolls the page from its top until found(im) gives a hit inside the content box's middle part."""
    wheel(win, 4, 30)
    top, bottom = win.content[1] + 30, win.content[3] - 50
    for _ in range(notches):
        im = win.shot('page')
        hit = found(im)
        if hit and top <= hit[1] <= bottom:
            return hit, im
        wheel(win, 5)
    raise Fail(f'{what} found on the page')


def label_at(win, label):
    def f(im):
        for words in readings(im, win.content):
            box = find_phrase(words, label)
            if box:
                return box
    return seek(win, f, f'the row "{label}"')


def band_words(win, im, box):
    return readings(im, (box[0], box[1], win.content[2], box[3]))


def row_band(win, lab):
    return (win.content[0], max(win.content[1], int(lab[1]) - 10), win.content[2], min(win.content[3], int(lab[1]) + 50))


def rightmost_mark(im, x0, band):
    """Centre of the rightmost cluster of columns holding ink (> 30 from the band's commonest colour)."""
    import numpy as np
    a = np.asarray(im.crop((x0, band[1], band[2], band[3]))).astype(int)
    flat = a.reshape(-1, 3)
    vals, counts = np.unique(flat, axis=0, return_counts=True)
    bg = vals[counts.argmax()]
    ink = np.abs(a - bg).max(axis=2) > 30
    cols = np.where(ink.any(axis=0))[0]
    if not len(cols):
        return None
    end = cols[-1]
    start = end
    for c in reversed(cols[:-1]):
        if start - c <= 3:
            start = c
        else:
            break
    ys = np.where(ink[:, start:end + 1].any(axis=1))[0]
    return x0 + (start + end) / 2, band[1] + (ys[0] + ys[-1]) / 2


def switch(win, label, key, want, tag, absent_ok=False):
    lab, im = label_at(win, label)
    band = row_band(win, lab)
    target = None
    for words in band_words(win, im, band):
        seg = [w for w in words if w[1] > lab[2] + 8 and norm(w[0]) == ('on' if want else 'off')]
        other = [w for w in words if w[1] > lab[2] + 8 and norm(w[0]) == ('off' if want else 'on')]
        if seg and other:
            target = q1.centre(seg[-1])
            break
    if not target:
        target = rightmost_mark(im, int(lab[2]) + 40, band)
    require(target, f'{tag}: the "{label}" row has a switch')
    win.click(*target)
    expect(win, key, want, f'{tag} {label}', absent_ok)


def segment(win, label, word, key, want, tag, row_word=None):
    """Clicks the segment `word` on the row of `label` (or anywhere on the page when label is None)."""
    if label:
        lab, im = label_at(win, label)
        band = row_band(win, lab)
        x0 = lab[2] + 8
    else:
        lab, im = seek(win, lambda im: next((w[1:] for r in readings(im, win.content) for w in r
                                             if token_ok(w[0], word)), None), f'the segment "{word}"')
        band, x0 = (win.content[0], int(lab[1]) - 6, win.content[2], int(lab[3]) + 6), win.content[0]
    hit = None
    for words in band_words(win, im, band):
        c = [w for w in words if w[1] >= x0 and token_ok(w[0], word)]
        if c:
            hit = max(c, key=lambda w: w[1])
            break
    require(hit, f'{tag}: "{label or word}" has a "{word}" segment')
    win.click(*q1.centre(hit))
    expect(win, key, want, f'{tag} {label or ""} {word}'.replace('  ', ' '))


def field(win, label, shows, typed, key, want, tag, absent_ok=False):
    """Clicks the field on the row of label whose text satisfies shows(text), replaces its text, Enter."""
    lab, im = label_at(win, label)
    band = row_band(win, lab)
    hit = None
    for words in band_words(win, im, band):
        c = [w for w in words if w[1] > lab[2] + 6 and shows(w[0])]
        if c:
            hit = c[0]
            break
    require(hit, f'{tag}: the "{label}" field shows its value (read {all_text(im, band)[0][-80:]!r})')
    win.click(*q1.centre(hit))
    win.key('ctrl+a')
    win.type(typed)
    win.key('Return')
    expect(win, key, want, f'{tag} {label} "{typed}"', absent_ok)


def num_is(v):
    def f(t):
        t = re.sub(r'[^0-9.]', '', t)
        try:
            return abs(float(t) - v) < 1e-6
        except ValueError:
            return False
    return f


def text_has(s):
    return lambda t: s in t.lower()


def swatch_boxes(im, colour, box, tol=2, square=True):
    """Boxes of connected regions of colour (within tol) inside box, filtered to swatch shapes when square."""
    import numpy as np
    a = np.asarray(im.crop(box)).astype(int)
    mask = np.abs(a - np.array(colour)).max(axis=2) <= tol
    if mask.sum() > 300000:
        return []
    seen = np.zeros_like(mask)
    out = []
    for y, x in zip(*np.where(mask)):
        if seen[y, x]:
            continue
        q, n = deque([(y, x)]), 0
        seen[y, x] = True
        x0 = x1 = x
        y0 = y1 = y
        while q:
            cy, cx = q.popleft()
            n += 1
            x0, x1, y0, y1 = min(x0, cx), max(x1, cx), min(y0, cy), max(y1, cy)
            for ny, nx in ((cy + 1, cx), (cy - 1, cx), (cy, cx + 1), (cy, cx - 1)):
                if 0 <= ny < mask.shape[0] and 0 <= nx < mask.shape[1] and mask[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    q.append((ny, nx))
        w, h = x1 - x0 + 1, y1 - y0 + 1
        if square and not (10 <= w <= 64 and 10 <= h <= 64 and .6 <= w / h <= 1.7 and n >= .6 * w * h):
            continue
        out.append((box[0] + x0, box[1] + y0, box[0] + x1, box[1] + y1))
    return out


def find_swatch(win, colour, what):
    return seek(win, lambda im: next(iter(swatch_boxes(im, rgb(colour), win.content)), None), what)


def set_swatch(win, old, new, key, want, tag):
    box, _ = find_swatch(win, old, f'{tag}: a swatch of {old}')
    win.click((box[0] + box[2]) / 2, (box[1] + box[3]) / 2)
    time.sleep(.5)
    win.key('ctrl+a')
    win.type(new)
    win.key('Return')
    expect(win, key, want, f'{tag} swatch {old} -> {new}')


# ---------------------------------------------------------------- S0, S1, X1, I1, X2

def settings_story():
    win = start('settings', BASE, 'S0 a window starts on a config.xml holding the base settings')
    home = win.home
    want = expect_all(home)
    try:
        open_settings(win, 'S0')
        print('ok: S0 the window starts on config.xml; Settings has the seven groups')
        if FAULT not in ('colour', 'agent'):
            general_page(win)
        terminal_page(win)
        if FAULT != 'agent':
            editor_page(win)
        agents_page(win)
        history_page(win)
        open_settings(win, 'S1 Keys')
        page(win, 'Keys')
        im = win.shot('keys')
        texts = all_text(im, win.content)
        require(any('ctrl' in t and 'tab' in t for t in texts), f'S1 Keys lists the shortcuts (read {texts[0][:120]!r})')
        appearance_page(win)
        open_settings(win, 'S1 General')
        page(win, 'General')
        switch(win, 'Post-AI mode', 'post_ai', True, 'S1 General')
        print('ok: S1 every key set through Settings, saved and applied')
        got = final_check(win.config, want, 'X1')
        print('ok: X1 the final config.xml holds the 37 keys')
        q1.idle(win, 'I1 Settings open, after the last change')
        close_settings(win)
        q1.idle(win, 'I2 after Settings closes')
        print('ok: I1 I2 0 idle frames')
        text = win.config.read_text()
    finally:
        win.close()
    again = start('restart', text, 'X2 a window starts on the file Settings wrote')
    try:
        open_settings(again, 'X2')
        texts = all_text(again.shot('x2'), again.frame)
        require(not any(re.search(r'line\s*\d', t) for t in texts), f'X2 no "line N" error ({texts[0][-120:]!r})')
        time.sleep(3)
        require(again.config.read_text() == text, 'X2 the file is byte for byte the same 3 s after the start')
    finally:
        again.close()
    print('ok: X2 the written file starts a new window as is')
    return got


def general_page(win):
    open_settings(win, 'S1 General')
    page(win, 'General')
    tag = 'S1 General'
    switch(win, 'Restore layout', 'restore_layout', False, tag)
    switch(win, 'Robot greeting', 'startup_animation', True, tag, absent_ok=True)
    switch(win, 'Robot greeting', 'startup_animation', False, tag)
    switch(win, 'Animations', 'animations', True, tag, absent_ok=True)
    switch(win, 'Animations', 'animations', False, tag)
    field(win, 'Notify after', num_is(10), '26' if FAULT == 'typo' else '25', 'notify_after_seconds', 25, tag)
    field(win, 'Columns', num_is(80), '100', 'columns', 100, tag)
    field(win, 'Rows', num_is(24), '30', 'rows', 30, tag)
    print('ok: S1 General')


def terminal_page(win):
    open_settings(win, 'S1 Terminal')
    page(win, 'Terminal')
    tag = 'S1 Terminal'
    segment(win, 'Cursor blink', 'on', 'cursor_blink', 'on', tag)
    segment(win, 'Cursor blink', 'off', 'cursor_blink', 'off', tag)
    field(win, 'Blink cycle', num_is(1200), '800', 'cursor_blink_time_ms', 800, tag)
    field(win, 'Stop blinking after', num_is(10), '5', 'cursor_blink_timeout_s', 5, tag)
    switch(win, 'Clipboard writes', 'osc52_write', False, tag)
    switch(win, 'Clipboard reads', 'osc52_read', True, tag)
    cur = json.loads(json.dumps(OLD))
    for k in ('background', 'foreground', 'cursor'):
        cur[k] = NEW[k]
        set_swatch(win, OLD[k], NEW[k], 'terminal_colors', json.loads(json.dumps(cur)), f'{tag} {k}')
    for i in range(16):
        if FAULT == 'colour' and i == 5:
            continue
        cur['ansi'][i] = NEW['ansi'][i]
        set_swatch(win, OLD['ansi'][i], NEW['ansi'][i], 'terminal_colors', json.loads(json.dumps(cur)), f'{tag} ANSI {i}')
    expect(win, 'terminal_colors', NEW, f'{tag} all 19 colours')
    close_settings(win)
    win.type("clear; printf '" + ''.join(f'\\033[4{i}m  ' for i in range(8)) + "\\033[0m\\n"
             + ''.join(f'\\033[10{i}m  ' for i in range(8)) + "\\033[0m\\n'\n")
    time.sleep(1.5)
    win.away()
    im = win.shot('ansi')
    missing = [i for i, c in enumerate(NEW['ansi'])
               if not [b for b in swatch_boxes(im, rgb(c), (0, 40, W, H - 20), 2, False)
                       if b[2] - b[0] + 1 >= 8 and b[3] - b[1] + 1 >= 10]]
    require(not missing, f'{tag}: the pane paints the new ANSI colours (missing {missing})')
    px = im.getpixel((W - 60, 450))
    require(diff(px, rgb(NEW['background'])) <= 2, f'{tag}: the pane background is {NEW["background"]} (observed {px})')
    print('ok: S1 Terminal (19 colours drawn in the pane)')


def editor_page(win):
    open_settings(win, 'S1 Editor')
    page(win, 'Editor')
    tag = 'S1 Editor'
    field(win, 'Dictionary', text_has('en_us'), 'de_DE', 'compose_dictionary', 'de_DE', tag)
    field(win, 'Dictionary folder', lambda t: 'usr' in t.lower() or 'hunspell' in t.lower(), str(win.home / 'dicts'), 'compose_dictionary_dir',
          str(win.home / 'dicts'), tag)
    switch(win, 'Enter after send', 'compose_send_enter', True, tag)
    print('ok: S1 Editor')


def agents_page(win):
    open_settings(win, 'S1 Agents')
    page(win, 'Agents')
    tag = 'S1 Agents'
    segment(win, 'New agent panes', 'off', 'agent_pane_access', 'off', tag)
    add, _ = label_at(win, 'Add agent')
    win.click((add[0] + add[2]) / 2, (add[1] + add[3]) / 2)
    time.sleep(.5)
    win.type(FAKE['name'])
    win.key('Tab')
    win.type(FAKE['command'])
    win.key('Return')
    expect(win, 'agents', DEFAULT_AGENTS + [FAKE], f'{tag} Add agent')
    gone = 'codex' if FAULT == 'agent' else 'pi'
    row, im = seek(win, lambda im: next((w[1:] for r in readings(im, win.content) for w in r if norm(w[0]) == gone),
                                        None), f'{tag}: the "{gone}" row')
    band = (win.content[0], int(row[1]) - 16, win.content[2], int(row[3]) + 16)
    rm = None
    for words in band_words(win, im, band):
        c = [w for w in words if token_ok(w[0], 'remove')]
        if c:
            rm = c[0]
            break
    require(rm, f'{tag}: the "{gone}" row has a Remove button')
    win.click(*q1.centre(rm))
    expect(win, 'agents', [DEFAULT_AGENTS[0], DEFAULT_AGENTS[1], FAKE], f'{tag} Remove on the Pi row')
    close_settings(win)
    win.key('ctrl+shift+g')
    time.sleep(1.2)
    im = win.shot('picker')
    reads = [[norm(w[0]) for w in r] for r in readings(im, (0, 0, W, H))]
    require(any('fake' in r for r in reads) and not any('pi' in r for r in reads),
            f'{tag}: the agent picker lists Fake Tool and no Pi (read {" ".join(reads[0])[:160]!r})')
    win.key('Escape')
    time.sleep(.4)
    print('ok: S1 Agents')


def history_page(win):
    open_settings(win, 'S1 History')
    page(win, 'History')
    tag = 'S1 History'
    switch(win, 'Save history', 'history', False, tag)
    field(win, 'Keep for', num_is(90), '30', 'history_days', 30, tag)
    field(win, 'Output per command', num_is(256), '512', 'history_output_kib', 512, tag)
    print('ok: S1 History')


def appearance_page(win):
    tag = 'S1 Appearance'
    open_settings(win, tag)
    page(win, 'Appearance')
    centre_x = (win.frame[0] + win.frame[2]) / 2
    title0 = win.shot('title0').getpixel((900, 4))
    segment(win, 'Follow the system', 'on', 'theme', 'system', tag)

    def pick(word, left):
        def f(im):
            for r in readings(im, win.content):
                for w in r:
                    if token_ok(w[0], word) and ((w[1] + w[3]) / 2 < centre_x) == left:
                        return w[1:]
        return seek(win, f, f'{tag}: "{word}" in the {"dark" if left else "light"} list')[0]
    box = pick('mocha', True)
    win.click((box[0] + box[2]) / 2, (box[1] + box[3]) / 2)
    expect(win, 'theme_dark', 'catppuccin-mocha', f'{tag} Mocha')
    title1 = win.shot('title1').getpixel((900, 4))
    require(diff(title0, title1) > 6, f'{tag}: the title bar shows the Mocha theme ({title0} -> {title1})')
    box = pick('paper', False)
    win.click((box[0] + box[2]) / 2, (box[1] + box[3]) / 2)
    expect(win, 'theme_light', 'paper-light', f'{tag} Paper (light)')
    segment(win, 'Follow the system', 'off', 'theme', 'dark', tag)

    cols = lambda: win.info()['cols']
    rows = lambda: win.info()['rows']
    c0 = cols()
    segment(win, None, 'vertical', 'tab_style', 'vertical', tag)
    until(lambda: c0 - cols() >= 10, f'{tag}: Vertical takes >= 10 columns (from {c0})', 3)
    c1 = cols()
    segment(win, 'Tools panel', 'off', 'tools_panel', 'off', tag)
    until(lambda: cols() - c1 >= 10, f'{tag}: Tools panel Off gives >= 10 columns (from {c1})', 3)
    segment(win, None, 'recent', 'tools', ['files', 'agents'], tag)
    segment(win, None, 'classic', 'command_input', 'classic', tag)
    segment(win, None, 'cards', 'block_style', 'cards', tag)
    r0 = rows()
    segment(win, None, 'hide', 'status_line', 'hide', tag)
    until(lambda: rows() >= r0 + 1, f'{tag}: Hide gives >= 1 more row (from {r0})', 3)

    c2 = cols()
    field(win, 'Font size', num_is(15), '18', 'font_size', 18, tag)
    until(lambda: cols() <= .9 * c2, f'{tag}: font size 18 leaves <= 0.9 x {c2} columns', 3)
    strip = (240, 40, W, 140)
    pane0 = win.shot('pane0').crop(strip).tobytes()
    field(win, 'Terminal font', text_has('plex'), 'DejaVu Sans Mono', 'font_family', 'DejaVu Sans Mono', tag)
    until(lambda: win.shot('pane1').crop(strip).tobytes() != pane0, f'{tag}: the terminal font changes the pane', 3)
    navbox = (win.frame[0], win.frame[1], win.content[0] - 8, win.frame[3])
    nav0 = win.shot('nav0').crop(navbox).tobytes()
    field(win, 'Interface font', text_has('geist'), 'IBM Plex Sans', 'ui_font_family', 'IBM Plex Sans', tag)
    until(lambda: win.shot('nav1').crop(navbox).tobytes() != nav0, f'{tag}: the interface font changes Settings', 3)
    close_settings(win)  # the frame and nav may have moved with the new font: measure them again
    open_settings(win, tag)
    page(win, 'Appearance')
    c3 = cols()
    field(win, 'Cell width', num_is(1), '1.2', 'cell_width_scale', 1.2, tag)
    until(lambda: cols() <= .9 * c3, f'{tag}: cell width 1.2 leaves <= 0.9 x {c3} columns', 3)
    r1 = rows()
    field(win, 'Line height', num_is(1.25), '1.5', 'cell_height_scale', 1.5, tag)  # 1.25 is R1's default
    until(lambda: rows() <= .9 * r1, f'{tag}: line height 1.5 leaves <= 0.9 x {r1} rows', 3)

    set_swatch(win, ACCENT_OLD, ACCENT_NEW, 'colors', {'accent': ACCENT_NEW, 'err': ERR_OLD}, f'{tag} Window colours accent')
    find_swatch(win, ACCENT_NEW, f'{tag}: the accent swatch shows {ACCENT_NEW}')
    box, im = find_swatch(win, ERR_OLD, f'{tag}: the err swatch')
    cy = (box[1] + box[3]) / 2
    band = (win.content[0], int(cy) - 18, win.content[2], int(cy) + 18)
    reset = None
    for words in band_words(win, im, band):
        c = [w for w in words if token_ok(w[0], 'reset')]
        if c:
            reset = c[0]
            break
    require(reset, f'{tag}: the err row has a Reset button')
    win.click(*q1.centre(reset))
    expect(win, 'colors', {'accent': ACCENT_NEW}, f'{tag} Reset err')
    field(win, 'Opacity', num_is(1), '0.85', 'opacity', 0.85, tag)
    print('ok: S1 Appearance')


def final_check(path, want, tag, tol=1e-9):
    import xml.etree.ElementTree as ET
    raw = Path(path).read_text()
    try:
        root = ET.fromstring(raw)
    except ET.ParseError as e:
        raise Fail(f'{tag}: config.xml is well-formed XML ({e})')
    require(root.tag == 'clanky' and root.get('version') == '1', f'{tag}: root <clanky version="1"> ({root.tag} {root.attrib})')
    tags = [el.tag for el in root]
    require(len(tags) == len(set(tags)), f'{tag}: one element per setting ({[t for t in tags if tags.count(t) > 1]})')
    got = config_xml.loads(raw)
    extra, missing = sorted(set(got) - set(want)), sorted(set(want) - set(got))
    require(not extra and not missing, f'{tag}: exactly the expected keys (extra {extra}, missing {missing})')
    wrong = [k for k in want if not same(got[k], want[k], tol) or not (tol > 1e-9 or raw_ok(raw, k, want[k]))]
    require(not wrong, f'{tag}: values ({", ".join(f"{k}={short(got[k])}" for k in wrong[:4])})')
    el = {e.tag: e for e in root}
    if 'tools' in el:
        require(all(c.tag == 'tool' for c in el['tools']), f'{tag}: <tools> holds <tool> elements')
    if 'agents' in el:
        require(all(a.tag == 'agent' and [c.tag for c in a] in (['name', 'command'], ['command', 'name'])
                    for a in el['agents']), f'{tag}: <agents> holds <agent><name/><command/></agent>')
    if 'terminal_colors' in el:
        kids = {c.tag: c for c in el['terminal_colors']}
        require(set(kids) == {'background', 'foreground', 'cursor', 'ansi'} and len(kids['ansi']) == 16
                and all(c.tag == 'color' for c in kids['ansi']),
                f'{tag}: <terminal_colors> holds background, foreground, cursor and <ansi> with 16 <color>')
    return got


# ---------------------------------------------------------------- B1 B2 B3: bad files

GOOD_HEAD = '<?xml version="1.0" encoding="UTF-8"?>\n<clanky version="1">\n'


def shows_line(win, n, extra=None):
    for t in all_text(win.shot('err'), win.frame):
        if re.search(rf'line\s*:?\s*{n}\b', t) and (extra is None or extra in t):
            return True
    return False


def bad_story():
    text = GOOD_HEAD + '  <tab_style>vertical</tab_style>\n  <font_size>16</font_sizee>\n</clanky>\n'
    win = start('b1', text, 'B1 a malformed config.xml does not stop the window')
    try:
        open_settings(win, 'B1')
        require(shows_line(win, 4), f'B1 Settings shows the error at "line 4" (read {all_text(win.shot("b1"), win.frame)[0][-160:]!r})')
        time.sleep(3)
        require(win.config.read_text() == text, 'B1 the malformed file is not rewritten')
        page(win, 'General')
        lab, im = label_at(win, 'Restore layout')
        target = rightmost_mark(im, int(lab[2]) + 40, row_band(win, lab))
        require(target, 'B1 the Restore layout switch')
        win.click(*target)
        expect(win, 'restore_layout', False, 'B1 a change in Settings writes a well-formed file')
    finally:
        win.close()
    print('ok: B1 malformed file')
    text = GOOD_HEAD + '  <opacity>0.3</opacity>\n  <tab_style>vertical</tab_style>\n</clanky>\n'
    win = start('b2', text, 'B2 a bad value does not stop the window')
    try:
        open_settings(win, 'B2')
        require(shows_line(win, 3, 'opacity'), f'B2 Settings shows "line 3" and "opacity" '
                                               f'(read {all_text(win.shot("b2"), win.frame)[0][-160:]!r})')
        time.sleep(3)
        require(win.config.read_text() == text, 'B2 the file is not rewritten')
    finally:
        win.close()
    print('ok: B2 bad value')
    win = start('b3', dict(BASE, tab_style='vertical'), 'B3 a window on a good file')
    try:
        c0 = win.info()['cols']
        open_settings(win, 'B3')
        page(win, 'General')
        bad = config_xml.dump(dict(BASE, tab_style='horizontal')).splitlines(keepends=True)
        if FAULT != 'malformed':
            bad[4] = '  <rows>30</rowz>\n'
        bad = ''.join(bad)
        win.config.write_text(bad)
        until(lambda: shows_line(win, 5), 'B3 Settings shows the live error at "line 5"', 4)
        require(win.info()['cols'] == c0, f'B3 the last good values stay (columns {c0} -> {win.info()["cols"]})')
        time.sleep(3)
        require(win.config.read_text() == bad, 'B3 the broken file is not rewritten')
        lab, im = label_at(win, 'Restore layout')
        target = rightmost_mark(im, int(lab[2]) + 40, row_band(win, lab))
        require(target, 'B3 the Restore layout switch')
        win.click(*target)
        expect(win, 'restore_layout', False, 'B3 a change writes a well-formed file')
        expect(win, 'tab_style', 'vertical', 'B3 the written file keeps the last good values')
    finally:
        win.close()
    print('ok: B3 live error keeps the last good values')


# ---------------------------------------------------------------- M1-M4: migration

def migrate_window(name, source_bytes, label, xml=None):
    def prepare(w):
        d = w / 'config/clanky'
        d.mkdir(parents=True, exist_ok=True)
        (d / 'config.json').write_bytes(source_bytes)
        (d / 'config.json').chmod(0o600)
        if xml is not None:
            config_xml.write(d / 'config.xml', xml)
    if FAULT == 'migrate' and name == 'm1':
        return start(name, {}, label, prepare=prepare)
    return start(name, None, label, explicit=False, prepare=prepare)


def migrated(win, source_bytes, tag):
    d = win.w / 'config/clanky'
    until(lambda: (d / 'config.xml').exists(), f'{tag}: config.xml written within 5 s', 5)
    until(lambda: not (d / 'config.json').exists(), f'{tag}: config.json renamed', 5)
    require((d / 'config.json.migrated').exists() and (d / 'config.json.migrated').read_bytes() == source_bytes,
            f'{tag}: config.json.migrated is the old file byte for byte')
    return d / 'config.xml'


def migration_story():
    marc = MARC_CONFIG if MARC_CONFIG.is_file() else MARC_CONFIG_MIGRATED
    require(marc.is_file(), 'M1 the source config.json exists')
    source = marc.read_bytes()
    want = json.loads(source)
    if want.get('colors') == {}:
        want_keys = set(want) - {'colors'}
    else:
        want_keys = set(want)
    win = migrate_window('m1', source, 'M1 a window starts on a copy of the real config.json')
    try:
        path = migrated(win, source, 'M1')
        got = config_xml.load(path)
        require(set(got) - {'colors'} == want_keys - {'colors'} and ('colors' in got) <= ('colors' in want),
                f'M1 the same keys (extra {sorted(set(got) - set(want))}, missing {sorted(want_keys - set(got))})')
        wrong = [k for k in want_keys if not same(got.get(k), want[k], 1e-6)]
        require(not wrong and got.get('colors', {}) == {k: v.lower() for k, v in want.get('colors', {}).items()},
                f'M1 the same values (differ: {wrong})')
        open_settings(win, 'M1')
        texts = all_text(win.shot('m1'), win.frame)
        require(not any(re.search(r'line\s*\d', t) for t in texts), 'M1 Settings shows no error')
    finally:
        win.close()
    print('ok: M1 the real config migrates with the same values')
    legacy = {'font_size_pt': 12, 'rails': FAULT == 'legacy', 'columns': 90, 'theme': 'light'}
    src = json.dumps(legacy).encode()
    win = migrate_window('m2', src, 'M2 a window starts on a legacy config.json')
    try:
        final_check(migrated(win, src, 'M2'), dict(font_size=16, tools_panel='off', columns=90, theme='light'), 'M2', 1e-6)
    finally:
        win.close()
    print('ok: M2 legacy keys migrate to font_size and tools_panel')
    full = expect_all(ROOT / 'm3' / 'home')
    src = json.dumps(full).encode()
    win = migrate_window('m3', src, 'M3 a window starts on a config.json with every key')
    try:
        final_check(migrated(win, src, 'M3'), full, 'M3')
    finally:
        win.close()
    print('ok: M3 all 37 keys migrate')
    src = b'{"tab_style": "horizontal"}\n'
    win = migrate_window('m4', src, 'M4 a window starts with config.json and config.xml', xml={'tab_style': 'vertical'})
    try:
        d = win.w / 'config/clanky'
        xml_bytes = config_xml.dump({'tab_style': 'vertical'}).encode()
        time.sleep(3)
        require((d / 'config.json').read_bytes() == src and (d / 'config.xml').read_bytes() == xml_bytes
                and not (d / 'config.json.migrated').exists(), 'M4 with both files nothing is migrated or rewritten')
    finally:
        win.close()
    print('ok: M4 an existing config.xml wins')


# ---------------------------------------------------------------- D1 D2: documentation

def docs_story():
    import xml.etree.ElementTree as ET
    doc = (REPO / 'docs/config.md').read_text()
    blocks = re.findall(r'```xml\n(.*?)```', doc, re.S)
    require(blocks, 'D1 docs/config.md has an ```xml example')
    try:
        root = ET.fromstring(blocks[-1] if len(blocks) == 1 else max(blocks, key=len))
    except ET.ParseError as e:
        raise Fail(f'D1 the docs example is well-formed XML ({e})')
    tags = {el.tag for el in root}
    require(root.tag == 'clanky' and root.get('version') == '1', 'D1 the example has the root <clanky version="1">')
    require(set(KEYS) <= tags and not tags & {'font_size_pt', 'rails'},
            f'D1 the example holds the 37 keys (missing {sorted(set(KEYS) - tags)}, legacy {sorted(tags & {"font_size_pt", "rails"})})')
    print('ok: D1 the documented example')
    bad = []
    for f in sorted((REPO / 'docs').glob('*.md')):
        if f.name.startswith('design-'):
            continue
        for n, line in enumerate(f.read_text().splitlines(), 1):
            if 'config.json' in line and 'migrat' not in line.lower():
                bad.append(f'{f.name}:{n}')
    require(not bad, f'D2 docs name config.json only for migration ({bad[:6]})')
    require('config.xml' in doc and 'config.json.migrated' in doc, 'D2 docs/config.md names config.xml and config.json.migrated')
    print('ok: D2 docs moved to config.xml')


# ---------------------------------------------------------------- idle fault

def idle_story():
    win = start('idle', BASE, 'I1 a window starts on config.xml')
    try:
        if FAULT == 'idle':
            win.type('while :; do echo tick; sleep .4; done\n')
        open_settings(win, 'I1')
        page(win, 'History')
        switch(win, 'Save history', 'history', False, 'I1')
        q1.idle(win, 'I1 Settings open, after a change')
    finally:
        win.close()
    print('ok: I1 0 idle frames')


def main():
    for tool in ('Xvfb', 'xdotool', 'ffmpeg', 'tesseract'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True, mode=0o700)
    stories = {None: [settings_story, bad_story, migration_story, docs_story],
               'typo': [settings_story], 'colour': [settings_story], 'agent': [settings_story],
               'idle': [idle_story], 'malformed': [bad_story], 'migrate': [migration_story],
               'legacy': [migration_story]}
    require(FAULT in stories, f'known fault {FAULT}')
    try:
        for story in stories[FAULT]:
            story()
    finally:
        # the copy of the real config.json never stays behind
        for d in ('m1',):
            shutil.rmtree(ROOT / d, ignore_errors=True)
    if FAULT is None:
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: C1 every setting in Settings, config.xml')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: C1 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: C1 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
