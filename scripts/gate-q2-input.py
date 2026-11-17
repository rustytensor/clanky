#!/usr/bin/env python3
"""Q2 gate: the command input (design 2026-11-13, milestone Q2 "command input"; mockup Marc approved:
/srv/devops/build/clanky-redesign/mockup.html, sections 1 and 2, CSS .input .chips .chip .line .ghost .t-* .mode
.pop; his answer R1 yes: the command input is on by default).

Usage: gate-q2-input.py [--fault colours|ghost|aside|complete|idle|classic]
A fault changes the run's input on purpose; a correct build must then FAIL.

Harness: the Q1 gate's window (scripts/gate-q1-look.py Win, itself the T2 harness's Window and free_display), a real
Clanky window on a private Xvfb display from :100 up, 1400 x 900 px at scale 1, software Vulkan, IBM Plex Mono 11 pt
at cell_height_scale 1.25 (9 x 26 px cells [P]), cursor blink off, theme Graphite dark, TZ=UTC, tools panel on
(the default), history on. The shell is real bash 5.3 (SHELL=/bin/bash) with a home made by the gate whose ~/.bashrc
is Fedora's usual one: it sources /etc/bashrc (so /etc/profile.d/*.sh run, as on Marc's machine), puts ~/bin on
PATH, then sets PS1='USERPS1> ', exports RCMARK=sourced-rc and defines alias ll2='echo aliasok'. The window is
started with VTE_VERSION=8400 in its environment, as when Clanky is started from Ptyxis: /etc/profile.d/vte.sh then
adds its own OSC 133 marks unless Clanky keeps them out (notes/clanky-input.md finding 1). Screenshots leave out the
pointer (ffmpeg -draw_mouse 0). Keys and text come from xdotool, as a person types them. Outcomes are read from the
owner API (list_blocks: each block's command, output, exit code; terminal_info rows; permission_get), the screen text,
files the programs write, pixels sampled at measured places, the frame trace and config.xml.

Interface fixed by this gate: no new config keys. command_input "editor" (default) | "classic", as Q1 fixed it.
Clanky starts bash and zsh with its own OSC 133 integration (the user's rc file sourced first); no setup in the home.

Sources: [M] the approved mockup's CSS (explicit px and colours), [T] assets/themes/graphite-dark.json, [P] the T2
gate's Ptyxis/VTE cell measurement, [B] bash 5.3 / readline / GNU coreutils / git behaviour, [S] tree-sitter-bash
0.25.1's own parse with its upstream highlights query (tree-sitter-highlight 0.27.2, a reference run
/srv/devops/build/clanky-redesign/ts-ref, run 2026-11-13 on the line below; output in TS_CAPS), [X] xterm ctlseqs.
Graphite [T]: background #111111 (17,17,17), border #ffffff1a over it (41,41,41), foreground #fafafa,
muted_foreground #a3a3a3, ansi[2] #4ec27a, ansi[4] #5e9cf0, ansi[5] #c586e8, ansi[6] #3fc4c0.

Expected results, fixed before the work:
  A1 editor mode (default config): after the first prompt no screen row holds "USERPS1" (the shell's own PS1 is not
     shown; design). Still none after `cd ~/alpha`, `echo $RCMARK`, `ll2`.
  A2 the input sits at the bottom of the pane [M .input]: a hairline (41,41,41) +-6 across >= 90 % of x 300..1300 at
     a y in 787..803 (status line top hairline 874 [Q1 G2] minus the input's height 1 + 8 + 32 + 7 + line + 10 with
     the chip row 32 px tall (Marc's R1 of 2026-11-14: chips >= 32 px, Ptyxis's chips 32; was the mockup's 21) and a
     line of 16 [M .caret] to 26 px [P] = 74..84, +-3).
  A3 the caret: typing 10 spaces after one "x" moves it 10 cells to the right (cell width 5..14 px measured from the
     move), its rows lie between the hairline + 30 (8 + 21 + 7 [M] - 6) and 873.
  D1 the user's rc was sourced: `echo $RCMARK` prints sourced-rc, `ll2` prints aliasok (an alias, so bash is
     interactive and read ~/.bashrc) [B].
  D2 every command typed in the input and sent with Enter makes exactly one OSC 133 block whose command is the typed
     text and whose exit code is bash's $? [B], with VTE_VERSION=8400 in the environment (finding 1): checked on every
     command below. `false` exits 1, `bash -c 'exit 7'` exits 7, an empty Enter makes no block in 1.5 s.
  D3 Shift+Enter adds a line: "echo one", Shift+Enter, 'echo "two  three"', Enter is ONE block with the command
     'echo one\\necho "two  three"', output "one\\ntwo  three", exit 0 [B: one submission, the last status].
  D4 the block meta line replaces the prompt [M .meta]: after `clear` and `echo $RCMARK`, the pane above the input
     reads "alpha" (the folder ~/alpha) although no command there contains it.
  E1 mouse cursor placement: "echo abcdef", a click on the left fifth of cell 8 ("d"), "X", Enter runs
     "echo abcXdef" (output abcXdef).
  E2 selection: "echo 12345", Shift+Left twice, "Z" runs "echo 123Z"; "echo vwxyz", a drag from the left fifth of
     cell 6 to the left fifth of cell 9 selects "wxy", "Q" runs "echo vQz".
  E3 undo: "echo keepme", Shift+Home, BackSpace (one deletion), Ctrl+Z, Enter runs "echo keepme".
  S1 bash colours from tree-sitter [S, M .t-cmd .t-str .t-flag .t-var, T]: the line TS_LINE typed in the input (not
     run). Each token's pixels are fitted to every candidate colour C as bg + a (C - bg), a in 0..1 (the Q3 gate's
     rule); the best fit must be the expected colour with a weighted residual <= 6. Capture -> colour: function
     (command_name: grep, and wc after the pipe) ansi[4]; string ("two words") ansi[2]; property (NAME, HOME)
     ansi[6] [M .t-var cyan]; constant (-n, -l, words starting with "-") ansi[5] [M .t-flag violet; the mockup wins
     over Q3's constant = ansi 6 here because the mockup names flags explicitly]; comment (# note) muted
     foreground: its brightest pixel is grey (channels within 14) with its top channel in 110..175 (163 is
     #a3a3a3). Not judged: "=", "val" (mockup green, tree-sitter none), "$", "|" (operators).
  G1 grey ghost text from history: after running "echo ghostalpha one", typing "echo gh" shows ink in cells 8..18
     whose brightest pixel is grey (channels within 14), top channel 45..175 [M .ghost #4a4a4a .. T muted], and at
     least 40 below the brightest pixel of the typed "gh". Right then Enter runs "echo ghostalpha one"; "echo gh",
     End, Enter runs it again; "echo gh", Enter runs "echo gh" (Enter never takes the ghost).
  H1 Up recalls history: after "echo histone" and "echo histtwo", Up, Up, Enter runs "echo histone".
  C1 Tab completion, files relative to the pane's folder (~/alpha holds scripts/gate-exit.sh, gate-menu.sh,
     gate-mouse.sh, notes.txt; folders crates/ (x.toml) and docs/; files pa-file.txt, zzq-file.txt):
     a. "ls scripts/gate-m" Tab opens a list above the input reading gate-menu.sh and gate-mouse.sh, not gate-exit.sh,
        the kind "file" and the footer "Tab completes · Enter runs · Esc closes" [M .pop]; items in name order as
        readline lists them [B], the first highlighted; Tab then puts it in: Enter runs "ls scripts/gate-menu.sh"
        (output scripts/gate-menu.sh).
     b. Tab, Down, Tab, Enter runs "ls scripts/gate-mouse.sh".
     c. Tab, Escape closes the list (the names are gone from above the input) and Enter runs the text as typed,
        "ls scripts/gate-m", which ls answers with exit 2 [B: GNU ls, missing operand file].
     d. Tab, Enter runs the highlighted item: "ls scripts/gate-menu.sh". (Lead's reading B of notes finding 6: the
        mockup shows the list with the highlight over "Enter runs".)
     e. one candidate goes in at once, as readline does [B]: "cat scripts/gate-e" Tab Enter runs
        "cat scripts/gate-exit.sh" (output exit-fixture-content); a folder gets its slash: "ls cra" Tab Enter runs
        "ls crates/" (output x.toml).
  C2 git branches after git checkout/switch (~/alpha is a git repo on branch papaya with branches kiwi and mango;
     [B] git for-each-ref): "git checkout ma" Tab Enter runs "git checkout mango"; "git checkout pa" Tab Enter runs
     "git checkout papaya" (not the file pa-file.txt: branches only); "git switch ki" Tab Enter runs
     "git switch kiwi". Each exits 0.
  C3 commands from PATH for the first word (~/bin/zzqcmd-unique on the window's and the shell's PATH): "zzq" Tab
     Enter runs "zzqcmd-unique" (output zzq-ran), not the file zzq-file.txt.
  K1 chips [M .chip]: the input reads "alpha" (folder ~/alpha), "papaya" (branch), "bash", "Agent access", and the
     switch "Shell" "Agent" [M .mode]. The folder chip is found by its fill [M .chip, a faint raised fill]: the
     background 6 % of the way to the foreground [T] per channel, round(17 + 0.06 x (250 - 17)) = round(30.98) = 31,
     so (31,31,31) +-6, at rest (no hover border). It is 32 px tall (+-2) [Marc's R1 2026-11-14: chips >= 32 px,
     Ptyxis's chips 32, as in A2; was the mockup's 21], its top 9 px (+-3) below
     the input hairline [M 1 px border + 8 px padding], its left edge at x 258 (+-3) [Marc's R1 2026-11-14: tools
     panel 236 + gap between panes 8 + padding inside the pane 14 = 258]. After `git switch kiwi` the input reads
     "kiwi" within 3 s; after `cd ~/bravo` (no repository) it reads "bravo" and neither "kiwi" nor "papaya".
  K2 a click on the agent access switch (15 px right of the word "access" [M gap 6 + switch 18 / 2]) makes
     permission_get allow_agent true within 2 s; a second click makes it false.
  K3 with a second pane (palette "Split right"), each pane's input has its own "access" chip; a click on the right
     pane's switch sets allow_agent true for that pane only.
  K4 a click on "Agent" in the Shell | Agent switch opens the agent picker ("Start an agent" on screen within 3 s);
     Escape closes it. N1: the window and server processes then hold no TCP or UDP socket (/proc/<pid>/fd against
     /proc/net/{tcp,tcp6,udp,udp6}): Clanky makes no AI calls itself.
  R1 steps aside while a command runs: with a key logger running (the T2 gate's LOGGER, raw mode), the folder chip
     is dimmed (its brightest pixel at most bg + 0.8 x what it was at the prompt [M opacity .55]) or not drawn, and
     the input reads "running" or "keys" [M "A command is running: keys go to it."]; the keys a, b, Up, Tab, Escape,
     Return reach the program as exactly "ab" ESC[A TAB ESC CR [X normal cursor mode]. After "q" the block ends.
     "sleep 30", then Ctrl+C after 1 s, ends that block with exit 130 [B 128 + SIGINT] within 3 s.
  R2 alternate screen: with a program in ESC[?1049h logging its reads (the mouse gate's PROG), the chip is dimmed or
     not drawn, and "z", Up, Escape reach it as "z" ESC[A ESC. After it ends, "echo backagain" runs from the input
     and the chip is back to its prompt brightness (+-10).
  R3 no OSC 133: a pane whose shell is ~/bin/minish (a small Python shell that prints "minish$ " and never sends
     OSC 133): the input is dimmed or not drawn, and "echo shok" Enter reaches minish as the line "echo shok\\n"
     and prints shok.
  M1 command_input "classic": the screen shows "USERPS1>", no input hairline is drawn, the pane has 2..4 more rows
     than the editor window (63..73 px / 26 px rows, A2), and readline gets the keys: "echo clok" Enter, Up, Enter
     prints clok twice [B].
  M2 Settings > Appearance > Classic in the editor window: config.xml command_input "classic" within 3 s, the pane
     gains 2..4 rows within 3 s (same window process), and after "echo backclassic" Enter the last screen row with
     text reads "USERPS1>" within 5 s.
  I1 0 frames from 1 s to 3.5 s after the last key (the 0-idle-frames rule): at the empty prompt, with ghost
     text shown, and with the completion list open.
  Z1 zsh: /usr/bin/zsh must exist (it is not installed on this machine on 2026-11-13: notes finding 4). With
     SHELL=zsh and ~/.zshrc setting PROMPT='ZUSERPS1> ' and exporting ZRCMARK=sourced-zrc: no "ZUSERPS1" on screen,
     `echo $ZRCMARK` prints sourced-zrc, `false` exits 1, the D3 two-line command is one block.
On clanky-gpui (CLANKY_BIN, G2), from the Graphite dark theme file and the rulebook (rulebook-tokens.json), never
from the binary: background #141414 (20,20,20); the input's hairline is the border role, white at 9 % over it
(41,41,41) (rulebook:46); status line 32 px (h_status), so its top is at 868; the pane's inside ends at
868 - 8 (pane margin) - 1 (its edge) = 859. The input above that is 1 + 8 + 28 + 8 + 18 + 12 = 75 px: hairline, the
8 px step above the chips, chips 28 (h_chip, rulebook:99; today's 32), 8 px between chips and text (today's 7 on the
rulebook's spacing steps, rulebook:87), the text line of the cell height 18 (IBM Plex Mono 11 pt x 1.25, from the font
file) and the pane padding 12 (rulebook:103; today's 10). A2: the hairline at 859 - 75 = 784 (+-3). K1: chips are
outlined, no fill, a border in the input role, white at 13 % over the background (51,51,51) +-6 (rulebook:47, 232);
28 px tall (+-2), top 9 px below the hairline, left edge at 236 + 8 + 1 + 12 = 257 (side column, pane margin, edge,
pane padding). K3 splits with Ctrl+Shift+D (Split right; the palette comes in G3). M1: the input gives back 63..75
px (its 75 less the pane's bottom padding of 12 it replaces) of 18 px rows, so classic has 3..5 more rows. Moved: K4
(the agent picker) to G3, M2 (Settings) to G4; N1 is still checked, at the end of the main story.
Planted faults: colours (expects `wc` as a string), ghost (Left instead of Right in G1), aside (the logger is told to
quit before the keys are sent in R1), complete (C1 runs from ~ instead of ~/alpha), idle (one key typed inside I1's
quiet window), classic (M1's window is started with command_input "editor").
"""
import importlib.util, json, os, re, shutil, subprocess, sys, time
from pathlib import Path
import importlib.util as _config_xml_util
_config_xml_spec = _config_xml_util.spec_from_file_location('clanky_config_xml', Path(__file__).with_name('clanky_config_xml.py'))
config_xml = _config_xml_util.module_from_spec(_config_xml_spec)
_config_xml_spec.loader.exec_module(config_xml)  # config.xml writer and reader shared by the gate harnesses

ROOT = Path(os.environ.get('CLANKY_Q2_WORK', '/srv/devops/build/clanky-q2-gate-run'))
os.environ['CLANKY_Q1_WORK'] = str(ROOT)
spec = importlib.util.spec_from_file_location('q1', Path(__file__).with_name('gate-q1-look.py'))
q1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q1)
gm, t2 = q1.gm, q1.t2
Fail, require, until = q1.Fail, q1.require, q1.until
ocr_words, find_word, centre, near = q1.ocr_words, q1.find_word, q1.centre, q1.near
APP = t2.APP
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
REPO = Path(__file__).resolve().parents[1]
W, H = 1400, 900
BG, LINE, STATUS_TOP = (17, 17, 17), (41, 41, 41), 874
E = '\x1b'
THEME = json.loads((REPO / 'assets/themes/graphite-dark.json').read_text())
# Today's window: the hairline 787..803, chips filled, 32 px tall, left at 258; classic gives back 2..4 rows.
HAIR_RANGE, CHIP_H, CHIP_LEFT, CLASSIC_ROWS = (787, 803), 32, 258, (2, 3, 4)
GPUI = t2.gate_app.override() is not None
if GPUI:
    # clanky-gpui: the theme file's background and border role, the rulebook's heights, steps and padding (see the
    # docstring's paragraph on clanky-gpui for the sums).
    _tokens = json.loads((REPO / 'scripts/window-chrome/rulebook-tokens.json').read_text())
    _bg = tuple(int(THEME['colors']['background'][i:i + 2], 16) for i in (1, 3, 5))
    _white = lambda a: tuple(round(c + a * (255 - c)) for c in _bg)
    _px = lambda group, name: _tokens[group][name]['px']
    _cell_h = t2.gate_app.gpui_cell('IBM Plex Mono', 11, 1.25)[1]
    BG, LINE = _bg, _white(_tokens['pane']['border_alpha']['value'])
    STATUS_TOP = H - _px('heights', 'h_status')
    _inside_bottom = STATUS_TOP - _px('pane', 'margin') - 1
    CHIP_H = _px('heights', 'h_chip')
    _input = 1 + 8 + CHIP_H + 8 + _cell_h + _px('pane', 'padding')
    _hair = _inside_bottom - round(_input)
    HAIR_RANGE = (_hair - 3, _hair + 3)
    CHIP_BORDER = _white(_tokens['themes']['graphite-dark']['input']['alpha'])
    CHIP_LEFT = _tokens['pane']['side_column_width']['px'] + _px('pane', 'margin') + 1 + _px('pane', 'padding')
    _back = (_input - _px('pane', 'padding'), _input)
    CLASSIC_ROWS = tuple(range(int(_back[0] // _cell_h), int(-(-_back[1] // _cell_h)) + 1))


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


# K1: the chip fill, the background 6 % of the way to the foreground [T].
CHIP = tuple(round(b + 0.06 * (f - b))
             for b, f in zip(rgb(THEME['colors']['background']), rgb(THEME['colors']['foreground'])))
A = THEME['ansi']
CANDS = {'plain': rgb(THEME['colors']['foreground']), 'muted': rgb(THEME['colors']['muted_foreground']),
         'ansi1': rgb(A[1]), 'ansi2': rgb(A[2]), 'ansi3': rgb(A[3]), 'ansi4': rgb(A[4]), 'ansi5': rgb(A[5]),
         'ansi6': rgb(A[6]), 'ansi7': rgb(A[7])}
TS_LINE = 'NAME=val grep -n "two words" $HOME | wc -l # note'
# ts-ref bash on TS_LINE (both the highlighter and --first-wins print the same), one entry per character.
P, F, C, S, O, M = 'property', 'function', 'constant', 'string', 'operator', 'comment'
TS_CAPS = [P] * 4 + [None] * 5 + [F] * 4 + [None] + [C] * 2 + [None] + [S] * 11 + [None, O] + [P] * 4 + \
          [None, O, None] + [F] * 2 + [None] + [C] * 2 + [None] + [M] * 6
CAP_COLOUR = {F: 'ansi4', S: 'ansi2', P: 'ansi6', C: 'ansi5', M: 'comment'}
BASE = dict(q1.BASE, theme='dark', theme_dark='graphite-dark', history=True)
BASHRC = ('[ -f /etc/bashrc ] && . /etc/bashrc\nPATH="$HOME/bin:$PATH"\nPS1=\'USERPS1> \'\n'
          'export RCMARK=sourced-rc\nalias ll2=\'echo aliasok\'\n')
ZSHRC = "PROMPT='ZUSERPS1> '\nexport ZRCMARK=sourced-zrc\n"
MINISH = r'''#!/usr/bin/python3 -I
import json, os, subprocess, sys
log = os.path.join(os.path.dirname(os.path.abspath(__file__)), "minish.log")
while True:
    sys.stdout.write("minish$ "); sys.stdout.flush()
    line = sys.stdin.readline()
    if not line:
        break
    with open(log, "a") as f:
        f.write(json.dumps(line) + "\n")
    subprocess.run(line, shell=True)
'''
EDITOR_ROWS_SEEN = []  # the editor window's rows, for M1
ACTIVE = {}  # folder chip box and its brightest channel at the prompt, measured in the main story


class Win(q1.Win):
    """The Q1 window with a chosen shell, the gate's home files, ~/bin first on PATH and VTE_VERSION set."""

    def __init__(self, name, cfg, shell='/bin/bash'):
        self.shell = shell
        super().__init__(name, cfg, None)

    def _start(self, cfg, extra_env):
        self.home = self.w / 'home'
        make_home(self.home)
        shell = str(self.home / 'bin/minish') if self.shell == 'minish' else self.shell
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(('XDG_', 'CLANKY', 'WAYLAND', 'DBUS', 'GNOME', 'GSETTINGS', 'VTE'))}
        env.update(HOME=str(self.home), XDG_CONFIG_HOME=str(self.w / 'config'), XDG_DATA_HOME=str(self.w / 'data'),
                   XDG_STATE_HOME=str(self.w / 'state'), XDG_RUNTIME_DIR=str(self.w / 'runtime'), SHELL=shell,
                   TZ='UTC', GSETTINGS_BACKEND='memory', WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1',
                   LP_NUM_THREADS='8', VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json',
                   CLANKY_FRAME_TRACE=str(self.w / 'frames.csv'), VTE_VERSION='8400',
                   PATH=str(self.home / 'bin') + ':' + os.environ.get('PATH', '/usr/bin:/bin'))
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
                             cwd=str(self.home), stdout=subprocess.DEVNULL, stderr=t2.gate_app.window_log(self.err))
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
        time.sleep(2)
        self.t0 = time.time()
        self.focus()

    def focus(self):
        self.x('windowfocus', '--sync', self.wid)
        self.x('mousemove', '--window', self.wid, '700', '300', 'click', '1')
        self.away()

    def away(self):
        self.x('mousemove', '--window', self.wid, '120', '600')
        time.sleep(.3)


def make_home(home):
    home.mkdir(mode=0o700, exist_ok=True)
    (home / '.bashrc').write_text(BASHRC)
    (home / '.zshrc').write_text(ZSHRC)
    alpha = home / 'alpha'
    for d in ('scripts', 'crates', 'docs'):
        (alpha / d).mkdir(parents=True, exist_ok=True)
    for name in ('gate-exit.sh', 'gate-menu.sh', 'gate-mouse.sh', 'notes.txt'):
        (alpha / 'scripts' / name).write_text('exit-fixture-content\n' if name == 'gate-exit.sh' else 'x\n')
    (alpha / 'crates/x.toml').write_text('x\n')
    for name in ('pa-file.txt', 'zzq-file.txt'):
        (alpha / name).write_text('x\n')
    (home / 'bravo').mkdir(exist_ok=True)
    bin_ = home / 'bin'
    bin_.mkdir(exist_ok=True)
    (bin_ / 'zzqcmd-unique').write_text('#!/bin/sh\necho zzq-ran\n')
    (bin_ / 'minish').write_text(MINISH)
    for f in ('zzqcmd-unique', 'minish'):
        (bin_ / f).chmod(0o755)
    git = ['git', '-C', str(alpha), '-c', 'user.name=gate', '-c', 'user.email=gate@localhost']
    genv = dict(os.environ, HOME=str(home), GIT_CONFIG_NOSYSTEM='1')
    for args in (['init', '-q', '-b', 'papaya'], ['add', '-A'], ['commit', '-q', '-m', 'init'],
                 ['branch', 'mango'], ['branch', 'kiwi']):
        subprocess.run(git + args, env=genv, check=True, stdout=subprocess.DEVNULL)


# ---------------------------------------------------------------- reading the window

def screen_rows(win):
    return q1.screen_rows(win)


def blocks(win, tab=None):
    out, after = [], None
    while True:
        args = dict(tab_id=tab or win.tab, limit=64)
        if after:
            args['after'] = after
        r = win.o.call(1, 'list_blocks', **args)
        out += r['blocks']
        if not r.get('has_more') or not r['blocks']:
            return out
        after = r['blocks'][-1]['block_id']


def cmd(b):
    return (b.get('command') or '').strip()


def out(b):
    return (b.get('output') or '').replace('\r', '').strip()


def run(win, text, label, keys=('Return',), timeout=10):
    """Types text, presses keys; waits for exactly one new complete block and returns it."""
    n = len(blocks(win))
    if text:
        win.type(text)
    win.key(*keys)

    def done():
        b = blocks(win)
        return b[n:] if len(b) > n and b[-1]['complete'] else None
    try:
        until(done, 'x', timeout)
    except Fail:
        raise Fail(f'{label}: the command runs as a complete OSC 133 block within {timeout} s '
                   f'(blocks: {[(cmd(b), b["complete"]) for b in blocks(win)[n:]]}; last rows: {last_rows(win)})')
    time.sleep(.7)
    new = blocks(win)[n:]
    require(len(new) == 1, f'{label}: exactly one block per command (observed {[cmd(b) for b in new]})')
    return new[0]


def expect(win, text, want_cmd, label, want_out=None, code=0, keys=('Return',)):
    b = run(win, text, label, keys)
    require(cmd(b) == want_cmd, f'{label}: runs {want_cmd!r} (observed {cmd(b)!r})')
    if want_out is not None:
        require(out(b) == want_out, f'{label}: prints {want_out!r} (observed {out(b)!r})')
    if code is not None:
        require(b.get('exit_code') == code, f'{label}: exit {code} (observed {b.get("exit_code")})')
    return b


def no_block(win, keys, label):
    n = len(blocks(win))
    win.key(*keys)
    time.sleep(1.5)
    require(len(blocks(win)) == n, f'{label}: no block ({[cmd(b) for b in blocks(win)[n:]]})')


def last_rows(win):
    return [r.strip() for r in screen_rows(win) if r.strip()][-3:]


def hairline(im):
    px = im.load()
    xs = range(300, 1300, 3)
    for y in range(STATUS_TOP - 130, STATUS_TOP - 20):
        if sum(near(px[x, y], LINE, 6) for x in xs) >= .9 * len(xs):
            return y
    return None


def faint_words(im, box, scale=3, sat=40):
    """OCR of box with every pixel's distance from the background stretched (40 levels above it reads as full ink), so
    the mockup's faint meta text (#6b6b6b on #111111) reads as well as bright text. Without the stretch tesseract
    dropped the faint folder entirely; a meta line without the folder still reads no "alpha" (planted on a saved shot)."""
    from PIL import Image
    crop = im.crop(box).convert('L')
    hist = crop.histogram()
    bg = hist.index(max(hist))
    ink = crop.point(lambda v: 255 - min(255, abs(v - bg) * 255 // sat))
    return ocr_words(ink.convert('RGB'), (0, 0, ink.width, ink.height), scale, 11)


def band_words(win, im=None, name='band', scale=4):
    im = im or win.shot(name)
    return ocr_words(im, (236, win.hair + 1, W, STATUS_TOP), scale, 11)


def band_text(win, name='band'):
    return ' '.join(w[0] for w in band_words(win, None, name))


def ocr_soft(im, box, scale=4):
    """OCR with stretched contrast, for dimmed text."""
    from PIL import Image, ImageOps
    crop = ImageOps.autocontrast(im.crop(box).convert('L'), cutoff=1)
    if sum(crop.getdata()) / max(1, crop.width * crop.height) < 128:
        crop = ImageOps.invert(crop)
    crop = crop.resize((crop.width * scale, crop.height * scale), Image.LANCZOS)
    path = ROOT / 'ocr-soft.png'
    crop.save(path)
    return subprocess.run(['tesseract', str(path), '-', '--psm', '11'], capture_output=True, text=True,
                          timeout=60).stdout.lower()


def peak(im, box):
    px = im.load()
    return max(max(px[x, y]) for x in range(box[0], box[2]) for y in range(box[1], box[3]))


def aside_ok(win, im, label, must_draw):
    """The input stepped aside: its folder chip is dimmed to <= bg + 0.8 x its prompt brightness; with must_draw
    it must still be drawn (brightest > bg + 20, mockup section 2: "stays visible but dimmed"), else it may be gone."""
    require(ACTIVE, f'{label}: folder chip measured at the prompt first')
    got = peak(im, ACTIVE['box'])
    limit = BG[0] + .8 * (ACTIVE['peak'] - BG[0])
    require(got <= limit, f'{label}: the input is dimmed (folder chip brightest {got}, at the prompt '
                          f'{ACTIVE["peak"]}, limit {limit:.0f})')
    if must_draw:
        require(got > BG[0] + 20, f'{label}: the input stays visible, dimmed (folder chip brightest {got})')


def idle(win, label, seconds=2.5, poke=False):
    while (time.time() - win.t0) % 60 > 60 - seconds - 4:
        time.sleep(1)
    t = time.time()
    if poke:
        time.sleep(2)
        win.x('type', 'x')
        time.sleep(seconds - .7)
    else:
        time.sleep(seconds + 1.3)
    late = q1.frames_between(win, t + 1, t + 1 + seconds)
    require(not late, f'{label}: 0 frames while nothing changes (observed {len(late)} in {seconds} s)')


# ---------------------------------------------------------------- colour judging (the Q3 gate's blend fit)

def cells(win, i0, i1):
    x0, cw, ya, yb = win.geo
    return int(x0 + i0 * cw) + 1, int(x0 + i1 * cw) - 1, ya - 3, yb + 4


def ink(im, box, bg=BG):
    px = im.load()
    return [px[x, y] for x in range(box[0], box[1]) for y in range(box[2], box[3])
            if max(abs(a - b) for a, b in zip(px[x, y], bg)) >= 12]


def fit(pixels, bg=BG):
    best = None
    for name, c in CANDS.items():
        d = [c[i] - bg[i] for i in range(3)]
        dd = sum(v * v for v in d) or 1
        tot = wsum = 0.0
        for p in pixels:
            q = [p[i] - bg[i] for i in range(3)]
            a = min(1.0, max(0.0, sum(q[i] * d[i] for i in range(3)) / dd))
            r = sum((q[i] - a * d[i]) ** 2 for i in range(3)) ** .5
            w = sum(v * v for v in q) ** .5
            tot += r * w
            wsum += w
        res = tot / (wsum or 1)
        if best is None or res < best[1]:
            best = (name, res)
    return best


def brightest(pixels):
    p = max(pixels, key=max)
    return p, max(p), max(p) - min(p)


def tokens():
    out, i = [], 0
    while i < len(TS_CAPS):
        j = i
        while j < len(TS_CAPS) and TS_CAPS[j] == TS_CAPS[i]:
            j += 1
        if TS_CAPS[i] in CAP_COLOUR:
            out.append((TS_LINE[i:j], i, j, CAP_COLOUR[TS_CAPS[i]]))
        i = j
    return out


# ---------------------------------------------------------------- the main window

def prompt_step(win):
    time.sleep(1)
    rows = screen_rows(win)
    require(not any('USERPS1' in r for r in rows), f'A1 editor mode: the shell\'s own PS1 is not shown (rows {last_rows(win)})')
    im = win.shot('start')
    win.hair = hairline(im)
    require(win.hair is not None and HAIR_RANGE[0] <= win.hair <= HAIR_RANGE[1],
            f'A2 the input\'s top hairline {LINE} at y {HAIR_RANGE[0]}..{HAIR_RANGE[1]} (observed {win.hair})')
    win.type('x')
    time.sleep(.6)
    a = win.shot('probe-a')
    win.type(' ' * 10)
    time.sleep(.6)
    b = win.shot('probe-b')
    pa, pb = a.load(), b.load()
    ys = range(win.hair + 2, STATUS_TOP)
    hit = [(x, y) for x in range(240, 1300) for y in ys if max(abs(i - j) for i, j in zip(pa[x, y], pb[x, y])) > 40]
    colsx = sorted({x for x, _ in hit})
    clusters = []
    for x in colsx:
        if clusters and x - clusters[-1][1] <= 2:
            clusters[-1][1] = x
        else:
            clusters.append([x, x])
    require(len(clusters) >= 2, f'A3 the caret moves when spaces are typed (changed columns {clusters[:4]})')
    (l0, l1), (r0, _) = clusters[0], clusters[1]
    cw = (r0 - l0) / 10
    yl = [y for x, y in hit if l0 <= x <= l1]
    ya, yb = min(yl), max(yl)
    require(5 <= cw <= 14, f'A3 the caret moves 10 cells for 10 spaces (cell {cw:.2f} px)')
    require(ya >= win.hair + 30 and yb <= STATUS_TOP - 1,
            f'A3 the text line lies below the chips: rows {ya}..{yb}, hairline {win.hair}')
    win.geo = (l0 - cw, cw, ya, yb)
    win.key(*['BackSpace'] * 11)
    print(f'ok: A1 A2 A3 no PS1, input at y {win.hair}, text cells {cw:.2f} px at x {l0 - cw:.0f}, rows {ya}..{yb}')


def rc_step(win):
    expect(win, 'cd ~/alpha', 'cd ~/alpha', 'D2 cd ~/alpha')
    expect(win, 'echo $RCMARK', 'echo $RCMARK', 'D1 the user\'s ~/.bashrc was sourced', 'sourced-rc')
    expect(win, 'll2', 'll2', 'D1 aliases from ~/.bashrc work', 'aliasok')
    expect(win, 'clear', 'clear', 'D4 clear')
    expect(win, 'echo $RCMARK', 'echo $RCMARK', 'D4 a block after clear', 'sourced-rc')
    im = win.shot('meta')
    text = ' '.join(w[0] for w in faint_words(im, (236, 40, W, win.hair - 1)))
    require('alpha' in text, f'D4 the block meta line shows the folder "alpha" (read {text[:120]!r})')
    require(not any('USERPS1' in r for r in screen_rows(win)), 'A1 still no PS1 after commands')
    print('ok: D1 D4 rc sourced, meta line instead of the prompt')


def filled_chip(im, folder, yc, hair):
    """Today's chip, found by its fill: its left edge, top and bottom rows."""
    px = im.load()
    vfill = lambda x: all(near(px[x, y], CHIP, 6) for y in (yc - 4, yc, yc + 4))
    start = int(folder[1]) - 2
    out = next((x for x in range(start, 237, -1) if not vfill(x)), None) if vfill(start) else None
    left = out + 1 if out else None
    require(left and abs(left - CHIP_LEFT) <= 3,
            f'K1 the folder chip\'s left edge is at x {CHIP_LEFT}, fill {CHIP} (observed {left})')
    hfill = lambda y: near(px[left + 8, y], CHIP, 6) and near(px[left + 9, y], CHIP, 6)
    above = next((y for y in range(yc, hair, -1) if not hfill(y)), None)
    below = next((y for y in range(yc, STATUS_TOP) if not hfill(y)), None)
    return left, above and above + 1, below and below - 1


def outlined_chip(im, folder, yc, hair):
    """clanky-gpui's chip, found by its border with the background inside: its left edge, top and bottom rows."""
    px = im.load()
    start = int(folder[1]) - 2
    left = next((x for x in range(start, 237, -1) if near(px[x, yc], CHIP_BORDER, 6)), None)
    inside = left is not None and all(near(px[x, yc], BG, 6) for x in range(left + 1, start))
    require(left and inside and abs(left - CHIP_LEFT) <= 3,
            f'K1 the folder chip\'s left edge {CHIP_BORDER} at x {CHIP_LEFT}, background inside (observed {left})')
    x = left + 14  # clear of the 6 px corner
    top = next((y for y in range(yc, hair, -1) if near(px[x, y], CHIP_BORDER, 6)), None)
    bot = next((y for y in range(yc, STATUS_TOP) if near(px[x, y], CHIP_BORDER, 6)), None)
    return left, top, bot


def chips_step(win):
    im = win.shot('chips')
    words = band_words(win, im)
    text = ' '.join(w[0] for w in words)
    for want in ('alpha', 'papaya', 'bash', 'access', 'shell', 'agent'):
        require(want in text, f'K1 the input reads {want!r} (read {text!r})')
    folder = find_word(words, lambda t: 'alpha' in t)
    yc = int(centre(folder)[1])
    left, top, bot = (outlined_chip if GPUI else filled_chip)(im, folder, yc, win.hair)
    require(top and bot and abs((bot - top + 1) - CHIP_H) <= 2,
            f'K1 the folder chip is {CHIP_H} px tall (edges {top}, {bot})')
    require(abs(top - win.hair - 9) <= 3, f'K1 the chip top is 9 px below the input hairline ({top} vs {win.hair})')
    box = (left + 2, top + 2, int(folder[3]) + 2, bot - 1)
    ACTIVE.update(box=box, peak=peak(im, box))
    expect(win, 'git switch kiwi', 'git switch kiwi', 'K1 git switch kiwi')
    until(lambda: 'kiwi' in band_text(win), 'K1 the branch chip reads "kiwi" within 3 s', 3)
    expect(win, 'cd ~/bravo', 'cd ~/bravo', 'K1 cd ~/bravo')
    until(lambda: (lambda t: 'bravo' in t and 'kiwi' not in t and 'papaya' not in t)(band_text(win)),
          f'K1 in ~/bravo the input reads "bravo" and no branch (read {band_text(win)!r})', 3)
    expect(win, 'cd ~/alpha', 'cd ~/alpha', 'K1 cd ~/alpha')
    expect(win, 'git switch papaya', 'git switch papaya', 'K1 git switch papaya')
    words = band_words(win)
    acc = find_word(words, lambda t: t == 'access')
    require(acc, 'K2 the agent access chip')
    sw = (acc[3] + 15, centre(acc)[1])
    win.click(*sw)
    until(lambda: win.o.call(3, 'permission_get', tab_id=win.tab)['allow_agent'] is True,
          'K2 a click on the agent access switch allows the agent', 2)
    win.click(*sw)
    until(lambda: win.o.call(3, 'permission_get', tab_id=win.tab)['allow_agent'] is False,
          'K2 a second click takes it back', 2)
    win.away()
    print('ok: K1 K2 chips, branch refresh, agent access switch')


def edit_step(win):
    x0, cw, ya, yb = win.geo
    ymid = (ya + yb) // 2
    win.type('echo abcdef')
    win.click(x0 + 8.2 * cw, ymid)
    win.type('X')
    expect(win, '', 'echo abcXdef', 'E1 a click places the cursor', 'abcXdef')
    win.type('echo 12345')
    win.key('shift+Left', 'shift+Left')
    expect(win, 'Z', 'echo 123Z', 'E2 Shift+Left selects')
    win.type('echo vwxyz')
    win.x('mousemove', '--window', win.wid, str(int(x0 + 6.2 * cw)), str(ymid), 'mousedown', '1')
    time.sleep(.2)
    win.x('mousemove', '--window', win.wid, str(int(x0 + 7.5 * cw)), str(ymid))
    time.sleep(.2)
    win.x('mousemove', '--window', win.wid, str(int(x0 + 9.2 * cw)), str(ymid), 'mouseup', '1')
    time.sleep(.3)
    expect(win, 'Q', 'echo vQz', 'E2 a mouse drag selects')
    win.type('echo keepme')
    win.key('shift+Home', 'BackSpace', 'ctrl+z')
    expect(win, '', 'echo keepme', 'E3 Ctrl+Z undoes')
    win.type('echo one')
    win.key('shift+Return')
    expect(win, 'echo "two  three"', 'echo one\necho "two  three"', 'D3 Shift+Enter adds a line', 'one\ntwo  three')
    expect(win, 'false', 'false', 'D2 false', code=1)
    expect(win, "bash -c 'exit 7'", "bash -c 'exit 7'", 'D2 exit 7', code=7)
    no_block(win, ['Return'], 'D2 an empty Enter')
    win.away()
    print('ok: E1 E2 E3 D2 D3 editing, multi-line, exit codes')


def colour_step(win):
    win.type(TS_LINE)
    time.sleep(.8)
    im = win.shot('colours')
    for text, i0, i1, want in tokens():
        if FAULT == 'colours' and text == 'wc':
            want = 'ansi2'
        if text.startswith('$'):
            continue
        px = ink(im, cells(win, i0, i1))
        require(len(px) >= 3, f'S1 token {text!r} is drawn in the input')
        if want == 'comment':
            p, top, spread = brightest(px)
            require(spread <= 14 and 110 <= top <= 175, f'S1 comment {text!r} is the muted foreground (brightest {p})')
            continue
        name, res = fit(px)
        require(name == want and res <= 6, f'S1 {text!r} is {want} {CANDS[want]} (fits {name} with residual {res:.1f})')
    win.key('shift+Home', 'BackSpace')
    no_block(win, ['Return'], 'S1 the line was cleared')
    print('ok: S1 tree-sitter colours')


def ghost_step(win):
    expect(win, 'echo ghostalpha one', 'echo ghostalpha one', 'G1 seed', 'ghostalpha one')
    win.type('echo gh')
    time.sleep(.8)
    im = win.shot('ghost')
    g = ink(im, cells(win, 8, 19))
    typed = ink(im, cells(win, 5, 7))
    require(len(g) >= 10 and typed, f'G1 ghost text is drawn after "echo gh" ({len(g)} ink pixels)')
    p, top, spread = brightest(g)
    _, ttop, _ = brightest(typed)
    require(spread <= 14 and 45 <= top <= 175 and top <= ttop - 40,
            f'G1 the ghost text is grey and dimmer than typed text (ghost brightest {p}, typed {ttop})')
    idle(win, 'I1 with ghost text shown')
    win.key('Left' if FAULT == 'ghost' else 'Right')
    expect(win, '', 'echo ghostalpha one', 'G1 Right accepts the ghost text', 'ghostalpha one')
    win.type('echo gh')
    time.sleep(.5)
    expect(win, '', 'echo ghostalpha one', 'G1 End accepts the ghost text', keys=('End', 'Return'))
    expect(win, 'echo gh', 'echo gh', 'G1 Enter never takes the ghost text', 'gh')
    print('ok: G1 I1 ghost text')


def history_step(win):
    expect(win, 'echo histone', 'echo histone', 'H1 seed')
    expect(win, 'echo histtwo', 'echo histtwo', 'H1 seed')
    expect(win, '', 'echo histone', 'H1 Up recalls history', 'histone', keys=('Up', 'Up', 'Return'))
    print('ok: H1 Up recalls history')


def popup_text(win, name):
    _, _, ya, _ = win.geo
    return ' '.join(w[0] for w in ocr_words(win.shot(name), (236, 120, W, ya - 2), 3, 11))


def complete_step(win):
    if FAULT == 'complete':
        expect(win, 'cd ~', 'cd ~', 'C1 fault: cd ~')
    expect(win, 'clear', 'clear', 'C1 clear')
    win.type('ls scripts/gate-m')
    win.key('Tab')
    time.sleep(.8)
    text = popup_text(win, 'popup')
    require('menu' in text and 'mouse' in text and 'exit' not in text,
            f'C1a Tab lists gate-menu.sh and gate-mouse.sh only (read {text!r})')
    require('file' in text and 'completes' in text, f'C1a the list shows the kind "file" and the footer (read {text!r})')
    idle(win, 'I1 with the completion list open')
    expect(win, '', 'ls scripts/gate-menu.sh', 'C1a Tab completes', 'scripts/gate-menu.sh', keys=('Tab', 'Return'))
    win.type('ls scripts/gate-m')
    expect(win, '', 'ls scripts/gate-mouse.sh', 'C1b Down then Tab', 'scripts/gate-mouse.sh',
           keys=('Tab', 'Down', 'Tab', 'Return'))
    expect(win, 'clear', 'clear', 'C1c clear')
    win.type('ls scripts/gate-m')
    win.key('Tab', 'Escape')
    time.sleep(.6)
    text = popup_text(win, 'escaped')
    require('menu' not in text and 'mouse' not in text, f'C1c Escape closes the list (read {text!r})')
    expect(win, '', 'ls scripts/gate-m', 'C1c Enter runs the text as typed', code=2)
    win.type('ls scripts/gate-m')
    expect(win, '', 'ls scripts/gate-menu.sh', 'C1d Enter with the list open', keys=('Tab', 'Return'))
    win.type('cat scripts/gate-e')
    expect(win, '', 'cat scripts/gate-exit.sh', 'C1e one candidate goes in at once', 'exit-fixture-content',
           keys=('Tab', 'Return'))
    win.type('ls cra')
    expect(win, '', 'ls crates/', 'C1e a folder gets its slash', 'x.toml', keys=('Tab', 'Return'))
    win.type('git checkout ma')
    expect(win, '', 'git checkout mango', 'C2 branch after git checkout', keys=('Tab', 'Return'))
    win.type('git checkout pa')
    expect(win, '', 'git checkout papaya', 'C2 branches only, not files', keys=('Tab', 'Return'))
    win.type('git switch ki')
    expect(win, '', 'git switch kiwi', 'C2 branch after git switch', keys=('Tab', 'Return'))
    expect(win, 'git switch papaya', 'git switch papaya', 'C2 back to papaya')
    win.type('zzq')
    expect(win, '', 'zzqcmd-unique', 'C3 commands from PATH for the first word', 'zzq-ran', keys=('Tab', 'Return'))
    print('ok: C1 C2 C3 I1 completion')


def wait_ended(win, n, label, timeout=5):
    until(lambda: len(blocks(win)) > n and blocks(win)[-1]['complete'], f'{label}: the block ends', timeout)
    time.sleep(.8)


def aside_step(win):
    logger, log = win.w / 'logger.py', win.w / 'keys.log'
    logger.write_text(t2.LOGGER)
    n = len(blocks(win))
    win.type(f'python3 {logger} {log}')
    win.key('Return')
    until(lambda: any('LOGGER-READY' in r for r in screen_rows(win)), f'R1 the key logger started ({last_rows(win)})')
    time.sleep(.8)
    im = win.shot('running')
    aside_ok(win, im, 'R1 while a command runs', True)
    said = ocr_soft(im, (236, win.hair - 2, W, STATUS_TOP))
    require('running' in said or 'keys' in said, f'R1 the input says where keys go (read {said.strip()[:100]!r})')
    if FAULT == 'aside':
        win.type('q')
        time.sleep(1)
    win.type('ab')
    win.key('Up', 'Tab', 'Escape', 'Return')
    time.sleep(1)
    got = ''.join(json.loads(l) for l in log.read_text().splitlines()) if log.exists() else ''
    want = 'ab' + E + '[A\t' + E + '\r'
    require(got == want, f'R1 keys reach the running program as {want!r} (observed {got!r})')
    win.type('q')
    wait_ended(win, n, 'R1 after q')
    n = len(blocks(win))
    win.type('sleep 30')
    win.key('Return')
    time.sleep(1.5)
    win.key('ctrl+c')
    until(lambda: len(blocks(win)) > n and blocks(win)[-1]['complete'] and blocks(win)[-1].get('exit_code') == 130,
          f'R1 Ctrl+C ends "sleep 30" with exit 130 within 3 s ({[(cmd(b), b.get("exit_code")) for b in blocks(win)[n:]]})', 3)
    time.sleep(.8)
    prog, log2 = win.w / 'prog.py', win.w / 'alt.log'
    prog.write_text(gm.PROG)
    n = len(blocks(win))
    win.type(f'python3 {prog} {log2} 1049 ""')
    win.key('Return')
    until(lambda: Path(str(log2) + '.size').exists(), 'R2 the alternate-screen program started')
    time.sleep(.8)
    aside_ok(win, win.shot('alt'), 'R2 on the alternate screen', False)
    win.type('z')
    win.key('Up', 'Escape')
    time.sleep(1)
    got = ''.join(json.loads(l) for l in log2.read_text().splitlines()) if log2.exists() else ''
    require(got == 'z' + E + '[A' + E, f'R2 keys reach the full-screen program (observed {got!r})')
    win.type('q')
    wait_ended(win, n, 'R2 after q')
    expect(win, 'echo backagain', 'echo backagain', 'R2 the input comes back', 'backagain')
    back = peak(win.shot('back'), ACTIVE['box'])
    require(abs(back - ACTIVE['peak']) <= 10, f'R2 the chip is back to its prompt brightness ({back} vs {ACTIVE["peak"]})')
    print('ok: R1 R2 the input steps aside')


def panes_step(win):
    if GPUI:
        win.key('ctrl+shift+d')
    else:
        win.key('ctrl+shift+p')
        time.sleep(.8)
        win.type('Split right')
        win.key('Return')
    until(lambda: len(win.terminals()) == 2, 'K3 "Split right" adds a pane')
    time.sleep(3)
    other = next(t for t in win.terminals() if t != win.tab)
    words = band_words(win, None, 'two-panes')
    acc = sorted((w for w in words if w[0] == 'access'), key=lambda w: w[1])
    require(len(acc) >= 2 and acc[0][1] < 820 < acc[-1][1], f'K3 each pane has its own access chip ({[w[1] for w in acc]})')
    win.click(acc[-1][3] + 15, centre(acc[-1])[1])
    until(lambda: win.o.call(3, 'permission_get', tab_id=other)['allow_agent'] is True,
          'K3 the right pane\'s switch allows the agent in that pane', 2)
    require(win.o.call(3, 'permission_get', tab_id=win.tab)['allow_agent'] is False, 'K3 the left pane stays off')
    win.o.call(1, 'close_tab', tab_id=other)
    until(lambda: len(win.terminals()) == 1, 'K3 second pane closed')
    time.sleep(1)
    win.focus()
    print('ok: K3 a chip on every pane')


def inet_sockets(pids):
    inodes = set()
    for name in ('tcp', 'tcp6', 'udp', 'udp6'):
        try:
            for line in Path(f'/proc/net/{name}').read_text().splitlines()[1:]:
                inodes.add(line.split()[9])
        except OSError:
            pass
    held = []
    for pid in pids:
        for fd in Path(f'/proc/{pid}/fd').glob('*'):
            try:
                m = re.match(r'socket:\[(\d+)\]', os.readlink(fd))
            except OSError:
                continue
            if m and m.group(1) in inodes:
                held.append((pid, m.group(1)))
    return held


def agent_step(win):
    words = band_words(win, None, 'mode')
    agent = find_word(words, lambda t: t == 'agent', pick=max)
    require(agent and agent[1] > 900, f'K4 the Shell | Agent switch ({[w for w in words if w[0] == "agent"]})')
    win.click(*centre(agent))
    until(lambda: 'start an agent' in ' '.join(w[0] for w in ocr_words(win.shot('picker'), (236, 40, W, STATUS_TOP), 3, 11)),
          'K4 "Agent" opens the agent picker ("Start an agent")', 3)
    no_network(win)
    win.key('Escape')
    time.sleep(.6)
    print('ok: K4 N1 agent picker, no network')


def no_network(win):
    held = inet_sockets([win.p.pid, win.server])
    require(not held, f'N1 no TCP or UDP sockets in the window or server ({held})')


def editor_rows_step(win):
    """clanky-gpui keeps its editor window's rows for M1 (today's window takes them in M2), and checks N1."""
    EDITOR_ROWS_SEEN.append(win.info()['rows'])
    no_network(win)
    print('ok: N1 no network')


def switch_classic_step(win):
    r0 = win.info()['rows']
    EDITOR_ROWS_SEEN.append(r0)  # M1 compares against the editor window's rows, so they are taken before M2 switches it
    q1.open_appearance(win)
    words = ocr_words(win.shot('layout'), (539, 200, 1065, 700), 3, 11)
    seg = find_word(words, lambda t: t == 'classic', pick=max)
    require(seg, 'M2 the Classic segment in Settings > Appearance')
    win.click(*centre(seg))
    until(lambda: win.saved().get('command_input') == 'classic', 'M2 config.xml command_input "classic"', 3)
    for _ in range(2):
        if q1.settings_open(win):
            win.key('Escape')
            time.sleep(.5)
    until(lambda: win.info()['rows'] - r0 in (2, 3, 4), f'M2 classic gives the rows back at once ({win.info()["rows"]} vs {r0})', 3)
    require(win.alive(), 'M2 same window process')
    win.focus()
    win.type('echo backclassic')
    win.key('Return')
    until(lambda: (last_rows(win) or [''])[-1].startswith('USERPS1>'),
          f'M2 the next prompt is the user\'s PS1 ({last_rows(win)})', 5)
    print('ok: M2 Classic applies at once')


def main_story():
    win = Win('editor', dict(BASE))
    try:
        steps = {None: [rc_step, chips_step, edit_step, colour_step, lambda w: idle(w, 'I1 at the empty prompt'),
                        ghost_step, history_step, complete_step, aside_step, panes_step]
                 + ([editor_rows_step] if GPUI else [agent_step, switch_classic_step]),
                 'colours': [colour_step], 'ghost': [ghost_step],
                 'idle': [lambda w: idle(w, 'I1 at the empty prompt', poke=True)],
                 'aside': [rc_step, chips_step, aside_step], 'complete': [rc_step, complete_step]}[FAULT]
        prompt_step(win)
        if FAULT not in (None, 'aside', 'complete'):
            expect(win, 'cd ~/alpha', 'cd ~/alpha', 'setup cd ~/alpha')
        for step in steps:
            step(win)
    finally:
        win.close()


def minish_story():
    win = Win('minish', dict(BASE), shell='minish')
    try:
        until(lambda: any('minish$' in r for r in screen_rows(win)), 'R3 minish started', 15)
        time.sleep(1)
        aside_ok(win, win.shot('minish'), 'R3 a shell without OSC 133', False)
        win.type('echo shok')
        win.key('Return')
        until(lambda: any(r.strip() == 'shok' for r in screen_rows(win)), f'R3 keys reach minish ({last_rows(win)})', 5)
        log = win.home / 'bin/minish.log'
        got = [json.loads(l) for l in log.read_text().splitlines()] if log.exists() else []
        require(got == ['echo shok\n'], f'R3 minish read exactly "echo shok\\n" (observed {got})')
    finally:
        win.close()
    print('ok: R3 no OSC 133')


def classic_story():
    win = Win('classic', dict(BASE, command_input='editor' if FAULT == 'classic' else 'classic'))
    try:
        time.sleep(1)
        require(any('USERPS1>' in r for r in screen_rows(win)), f'M1 classic shows the shell\'s PS1 ({last_rows(win)})')
        require(hairline(win.shot('classic')) is None, 'M1 classic draws no command input')
        rows = win.info()['rows']
        if EDITOR_ROWS_SEEN:
            e = EDITOR_ROWS_SEEN[0]
            require(rows - e in CLASSIC_ROWS,
                    f'M1 classic has {CLASSIC_ROWS[0]}..{CLASSIC_ROWS[-1]} more rows ({rows} vs editor {e})')
        win.type('echo clok')
        win.key('Return')
        until(lambda: sum(r.strip() == 'clok' for r in screen_rows(win)) == 1, 'M1 echo clok')
        win.key('Up', 'Return')
        until(lambda: sum(r.strip() == 'clok' for r in screen_rows(win)) == 2, f'M1 readline Up recalls ({last_rows(win)})', 5)
    finally:
        win.close()
    print('ok: M1 classic')


def zsh_story():
    zsh = shutil.which('zsh')
    require(zsh, 'Z1 zsh is installed (needed to check the zsh integration; it is missing on this machine)')
    win = Win('zsh', dict(BASE), shell=zsh)
    try:
        time.sleep(2)
        require(not any('ZUSERPS1' in r for r in screen_rows(win)), f'Z1 zsh: its own PROMPT is not shown ({last_rows(win)})')
        win.hair = hairline(win.shot('zsh')) or 0
        expect(win, 'echo $ZRCMARK', 'echo $ZRCMARK', 'Z1 zsh: ~/.zshrc was sourced', 'sourced-zrc')
        expect(win, 'false', 'false', 'Z1 zsh: false', code=1)
        win.type('echo one')
        win.key('shift+Return')
        expect(win, 'echo "two  three"', 'echo one\necho "two  three"', 'Z1 zsh: two lines, one block', 'one\ntwo  three')
    finally:
        win.close()
    print('ok: Z1 zsh')


def main():
    for tool in ('Xvfb', 'xdotool', 'ffmpeg', 'tesseract', 'git'):
        require(shutil.which(tool), f'tool {tool} installed')
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    stories = {None: [main_story, minish_story, classic_story, zsh_story], 'classic': [classic_story]}
    for story in stories.get(FAULT, [main_story]):
        story()
    if FAULT is None:
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: Q2 command input')


if __name__ == '__main__':
    known = (None, 'colours', 'ghost', 'aside', 'complete', 'idle', 'classic')
    try:
        require(FAULT in known, f'known fault {FAULT}')
        main()
    except Fail as e:
        print(f'FAIL: Q2 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: Q2 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
