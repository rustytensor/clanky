#!/usr/bin/env python3
"""Q3 gate: tree-sitter syntax colours in editor panes (design 2026-11-13, milestone Q3; approved
mockup, section 3 "Editor panes").

Usage: gate-q3-syntax.py [--fault table|theme|edit|italic|idle|language|oldmap]
A fault changes the run's input on purpose; the gate must then FAIL.

Runs in real Clanky windows on private Xvfb displays (:100 and up, the T2 harness's Window and free_display), at
1800x1000, default font and size, cursor blink off, rails hidden with Ctrl+Shift+B. The width holds the longest fixture
line (cursor_blink.rs line 2, 88 columns) in the half-width editor pane at Marc's R1 sizes (2026-11-14): tools panel
hidden, so the two panes share the window with one 8 px gap; each pane has 14 px of padding on both sides; cells are
9 px; the editor's line-number gutter takes 2 digits + 3 = 5 columns. Pane (1800 - 8) / 2 = 896 px, text columns
floor((896 - 28) / 9) - 5 = 96 - 5 = 91 >= 90 (at 1600: floor((796 - 28) / 9) - 5 = 80, which cut line 2 at column
81). Files are opened in an editor pane
with Ctrl+Shift+O, path, Enter, and closed with Ctrl+Shift+W. Screenshots are taken without the pointer
(ffmpeg x11grab -draw_mouse 0). Everything is judged from pixels, bytes on disk and the frame trace.

Interface fixed by this gate:
  Config keys: none new. Editor colours follow the window theme (`theme`, `theme_dark`, `theme_light`), never
  `terminal_colors` (that key keeps colouring terminals only: the mockup, "Ptyxis palettes keep working for the
  terminal"; the T2 gate, "the rest of the window keeps the theme").
  Language by file extension: .rs Rust, .py Python, .ts TypeScript, .tsx TSX, .js JavaScript, .go Go, .c C,
  .sh bash, .json JSON, .toml TOML, .yaml YAML, .md Markdown. Any other extension (.txt) stays plain.
  Token colours, from the design (theme JSON read straight from assets/themes/, not through Clanky's loader):
    keyword k = ansi[5]   function f = ansi[4]   type t = ansi[3]   string s = ansi[2]
    constant / macro c = ansi[6]   plain p = colors.foreground
    comment m = bg + 0.51 (colors.muted_foreground - bg) per channel, bg = colors.background, italic
    number n = 0.4 ansi[1] + 0.6 ansi[3] per channel
    (each channel rounded half up). comment and number are Marc's R2 "Mockup look" (2026-11-14, design section
    "Round 2", Q5); they replace comment = muted_foreground and number = ansi[1]. The mockup's own editor colours
    (mockup.html:129, Graphite) are .cm #5c5c5c and .nu #e8a87c; the formulas give these values, worked out by hand
    from the theme JSON before any build ran:
      theme           background  muted_foreground  -> comment   ansi[1]   ansi[3]   -> number
      stone-dark      #141210     #a0978d           -> #5b5650   #e2735f   #dcae5e   -> #de965e
      stone-light     #fbfaf8     #6b6259           -> #b2aca7   #a34737   #805e25   -> #8e552c
      graphite-dark   #111111     #a3a3a3           -> #5b5b5b   #f0716a   #e2b64f   -> #e89a5a
      graphite-light  #ffffff     #525252           -> #a7a7a7   #a34737   #805e25   -> #8e552c
    (graphite-dark: 17 + .51 (163 - 17) = 91.46 -> #5b, the mockup's #5c to within 1; .4 (240,113,106) +
    .6 (226,182,79) = (231.6, 154.4, 89.8) -> #e89a5a, the design's own figure.) An implementation that rounds
    differently is off by at most 1 per channel, far inside the judging residual of 3.
  Markdown (the design names the language only; the Q3 design notes): a heading's
  text is keyword, inline code and fenced code are string, prose is plain.

Fixtures (tests/syntax/, written for this gate, one per language). Each starts with two marker lines holding
U+2588 FULL BLOCK pairs at line 1 columns 7-8 and line 3 columns 27-28; the gate finds the four solid blocks in the
screenshot (colour-blind: any uniform rectangle) and gets the pane's cell size and origin from them.
  cursor_blink.rs: lines 12-26 are the mockup's editor section verbatim (mockup.html lines 347-361), so every class
  there is the mockup's own span class (kw k, fn f, ty t, nu n, mac c, cm m, no span p). This is where the mockup
  wins over upstream queries: `self` and `true` are keyword, `#[test]` and `assert!` are macro colour as whole spans,
  integers are number, and calls inside assert!( ) are function.
  Every other expected class (EXPECT below) is what the token is in tree-sitter's own parse of the fixture (the
  grammar crates in Cargo.lock, parsed 2026-11-13: function_declaration name, call function, type_identifier,
  primitive_type, string, number literal, comment, keyword token). The design's map then gives the colour. Where the
  grammars' upstream highlight queries disagree among themselves or with the design (JSON/TOML/YAML keys, JSX tags,
  bash words and expansions, Python `and`/`is not`, C #define bodies, a TSX component name, markdown link and fence
  markers) the token is "?" and not judged.

Judging a token: every pixel of its cells that differs from the pane background by at least 12 on some channel is
fitted to each candidate colour C as bg + a (C - bg), a clamped to 0..1 (glyph edges in Clanky's renderer are exactly
that blend: on 1.5.0 plain text fits with residual <= 1.1 at the 99th percentile). The token's colour is the
candidate with the smallest residual (weighted by how far each pixel is from bg); it must be the expected class with
a weighted residual <= 3. A comment is dimmer than the old comment colour and lies on the same line from bg (and,
within 2.3-3.4 per channel at full ink, on the line from bg to plain), so a colour fit alone cannot tell a
full-strength comment from muted or plain ink drawn faint. Each comment line must therefore also reach the comment
colour: over the line's comment tokens, the largest 99th percentile of a token's pixel a is >= 0.75 (the glyph cores
of a comment line are drawn at full ink: on 50af538 the "//" of cursor_blink.rs line 2 reaches 0.86 of the old
comment colour it is drawn in; comment ink drawn at 51% of a colour reaches at most 0.51 of it). Judged per line, not
per token, because a lone slanted "|" (line 8) may have no pixel fully covered. A token with fewer than 3 such pixels is not judged; at least 90% of a file's tokens must
be judged. A token with no ink at all reads "not drawn" (for example cut off at the pane edge).

Expected results, fixed before the work:
  C1 stone-dark window with terminal_colors set to Marc's Ptyxis "riley" palette: every judged token of all 12
     fixtures has its class colour from stone-dark.json (#c98ba6 k, #7ea3c4 f, #dcae5e t, #7fb685 s, #de965e n,
     #6fb5a8 c, #5b5650 m, #ebe6df p).
  C2 stone-light window: cursor_blink.rs and tokens.py match stone-light.json (#805c53 k, #526b7d f, #805e25 t,
     #426b4b s, #8e552c n, #416e63 c, #b2aca7 m, #2a2622 p).
  C3 italic comments: in cursor_blink.rs, the six lone `|` glyphs of the comment on line 8 lean right relative to
     the three lone plain `|` operators on line 10: (top-half ink centroid x - bottom-half ink centroid x) is larger
     for the comment by at least half of what a 9.4 degree slant gives over the distance between the two half
     centroids (9.4 is the smallest italicAngle among IBM Plex Mono Italic -9.5, JetBrains Mono Italic -9.4 and
     Geist Mono Italic -12.0, read from the fonts' post tables). Upright text gives exactly 0. The glyph is `|`, a
     straight bar in upright and italic faces alike, so it leans by exactly the face's angle: FreeType renders of
     GeistMono-Regular/-Italic.ttf v1.9.1 at 400 px give `|` 0.00 and 12.01 degrees with this metric. `l` does not:
     the italic face redraws its tail, and the same renders give `l` -5.09 and -1.91 degrees (3.2 apart, not 12).
  E1 incremental re-parse, on a copy of cursor_blink.rs, judged after each step:
     a. line 13 Home, type "// ": line 13 is all comment; line 18 still `impl` k, `Blink` t.
     b. BackSpace x3: line 13 is `pub struct` k, `Blink` t again.
     In a and b the caret would sit on a judged cell of line 13 (13:4, 13:1), so it is parked at line 1 column 1 for
     their screenshots and brought back after (the block caret redraws its cell in the cursor colour).
     c. line 11 (empty), type "const NN: u8 = 7;" and Enter: line 11 is const k, NN c, u8 t, 7 n; every mockup line
        moved down by one (12-26 -> 13-27) keeps its mockup classes.
     d. line 6 Home, type "/*"; line 7 End, type "*/": lines 6-7 are all comment; line 10 `fn tilt` is back to
        normal (fn k, tilt f, u8 t) and the mockup lines 13-27 keep their classes.
     e. Ctrl+S: the file on disk equals the fixture with exactly those edits (computed here in Python).
     f. with the caret parked at line 1 column 1, the colour of every judged cell equals that of a brand-new
        editor pane opened on the saved file (a fresh full parse is the reference for the incremental one).
  I1 0 frames (CLANKY_FRAME_TRACE "phases" lines) from 1 s to 4 s after the last key of step d (the design's
     "0 idle frames"; cursor blink off).
  P1 keystroke cost: 40 keys typed at 150 ms intervals at the end of line 40 of a 72 KB Rust file (studio.rs as of
     commit a818fac) and of the same bytes saved as .txt: the median of the frame trace's key-to-present latency is
     at most 5 ms higher for .rs (under a third of a 60 Hz frame; the design notes measured 0.43 ms re-parse plus
     0.42 ms viewport query in a release build). The median, not the 95th percentile: on 1.5.0 with software Vulkan
     both files are plain and their 95th percentiles still differed by 15 ms (68 vs 83 ms), while medians are ~50 ms
     with 1-2 ms spread.

Planted faults (each changes only the gate's input; a correct build must FAIL):
  table     the expected class of `Blink` on cursor_blink.rs line 13 flipped from type to keyword
  theme     C1 compares the dark window against stone-light.json
  edit      step a expects line 13 as comment but the harness types nothing
  italic    C3's "comment" sample is the plain `l`s of line 10 themselves
  idle      one extra key (Right) at 2.5 s inside I1's quiet window
  language  tokens.py is opened under the name tokens.py.txt but still judged as Python
  oldmap    C1 expects the old comment colour, muted_foreground itself (the map before R2); a build with the new
            map draws comments at 51% of it, so the first comment line fails the ink check (on a build with the old
            map it fails at the first number instead)
"""
import importlib.util, json, math, os, shutil, subprocess, sys, time
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[1]
ROOT = Path(os.environ.get('CLANKY_Q3_WORK', '/srv/devops/build/clanky-q3-gate-run'))
os.environ['CLANKY_T2_WORK'] = str(ROOT)
spec = importlib.util.spec_from_file_location('t2', Path(__file__).with_name('gate-t2-terminal.py'))
t2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t2)
Fail, require, until, Window, RILEY = t2.Fail, t2.require, t2.until, t2.Window, t2.RILEY
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None
FIX = REPO / 'tests/syntax'
W, H = 1800, 1000
t2.SCREEN = f'{W}x{H}x24'
BLOCK = '█'
SLANT_DEG = 9.4
PERF_COMMIT = 'a818fac'


def theme_colours(name, old=False):
    """Class colours straight from the theme JSON (the design's map, comment and number as of Marc's R2 2026-11-14).
    old=True gives the map before R2: comment = muted_foreground, number = ansi[1]."""
    d = json.loads((REPO / 'assets/themes' / f'{name}.json').read_text())
    rgb = lambda h: tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))
    mix = lambda x, y, t: tuple(math.floor(u + t * (v - u) + .5) for u, v in zip(x, y))
    a, c = d['ansi'], d['colors']
    pal = {'k': rgb(a[5]), 'f': rgb(a[4]), 't': rgb(a[3]), 's': rgb(a[2]), 'n': mix(rgb(a[1]), rgb(a[3]), .6),
           'c': rgb(a[6]), 'm': mix(rgb(c['background']), rgb(c['muted_foreground']), .51), 'p': rgb(c['foreground'])}
    if old:
        pal['m'], pal['n'] = rgb(c['muted_foreground']), rgb(a[1])
    return pal


# The values worked out by hand in the docstring; the gate stops if the formulas above ever disagree with them.
HAND = {'stone-dark': ('#5b5650', '#de965e'), 'stone-light': ('#b2aca7', '#8e552c'),
        'graphite-dark': ('#5b5b5b', '#e89a5a'), 'graphite-light': ('#a7a7a7', '#8e552c')}


NAMES = {'k': 'keyword', 'f': 'function', 't': 'type', 's': 'string', 'n': 'number', 'c': 'constant/macro',
         'm': 'comment', 'p': 'plain'}

# Expected classes: per line, (text, class) pieces that must join to exactly the fixture's line. "?" = not judged.
# Whitespace is never judged; a piece is split into tokens at spaces.
RUST = {
    2: [(None, 'm')],
    4: [('use', 'k'), (' std::time::', 'p'), ('Duration', 't'), (';', 'p')],
    6: [('const', 'k'), (' ', 'p'), ('LIMIT', 'c'), (': ', 'p'), ('u64', 't'), (' = ', 'p'), ('0x1F', 'n'), (';', 'p')],
    7: [('static', 'k'), (' ', 'p'), ('NAME', '?'), (': &', 'p'), ('str', 't'), (' = ', 'p'), ('"blink\\n"', 's'), (';', 'p')],
    8: [(None, 'm')],
    10: [('fn', 'k'), (' ', 'p'), ('tilt', 'f'), ('(l: ', 'p'), ('u8', 't'), (') -> ', 'p'), ('u8', 't'), (' { l | l | l | l }', 'p')],
    12: [(None, 'm')],
    13: [('pub struct', 'k'), (' ', 'p'), ('Blink', 't'), (' {', 'p')],
    14: [('    pub', 'k'), (' half: ', 'p'), ('Duration', 't'), (',', 'p')],
    15: [('    pub', 'k'), (' timeout: ', 'p'), ('Duration', 't'), (',', 'p')],
    16: [('}', 'p')],
    18: [('impl', 'k'), (' ', 'p'), ('Blink', 't'), (' {', 'p')],
    19: [('    pub fn', 'k'), (' ', 'p'), ('visible', 'f'), ('(&', 'p'), ('self', 'k'), (', since_key: ', 'p'),
         ('Duration', 't'), (') -> ', 'p'), ('bool', 't'), (' {', 'p')],
    20: [('        if', 'k'), (' since_key >= ', 'p'), ('self', 'k'), ('.timeout { ', 'p'), ('return true', 'k'), ('; }', 'p')],
    21: [('        (since_key.', 'p'), ('as_millis', 'f'), ('() / ', 'p'), ('self', 'k'), ('.half.', 'p'),
         ('as_millis', 'f'), ('()) % ', 'p'), ('2', 'n'), (' == ', 'p'), ('0', 'n')],
    22: [('    }', 'p')],
    23: [('}', 'p')],
    25: [('#[test]', 'c')],
    26: [('fn', 'k'), (' ', 'p'), ('stops_after_timeout', 'f'), ('() { ', 'p'), ('assert!', 'c'), ('(', 'p'),
         ('Blink', 't'), ('::', 'p'), ('vte', 'f'), ('().', 'p'), ('visible', 'f'), ('(', 'p'), ('Duration', 't'),
         ('::', 'p'), ('from_secs', 'f'), ('(', 'p'), ('11', 'n'), ('))); }', 'p')],
}
EXPECT = {
    'cursor_blink.rs': RUST,
    'tokens.py': {
        2: [(None, 'm')],
        4: [('import', 'k'), (' os', 'p')],
        6: [('def', 'k'), (' ', 'p'), ('greet', 'f'), ('(name: ', 'p'), ('str', 't'), (', count: ', 'p'), ('int', 't'),
            (' = ', 'p'), ('3', 'n'), (') -> ', 'p'), ('str', 't'), (':', 'p')],
        7: [('    """Say hello."""', 's')],
        8: [('    if', 'k'), (' count > ', 'p'), ('0', 'n'), (' ', 'p'), ('and', '?'), (' name ', 'p'), ('is not', '?'),
            (' ', 'p'), ('None', 'c'), (':', 'p')],
        9: [('        return', 'k'), (' ', 'p'), ('"hi "', 's'), (' + name * ', 'p'), ('2', 'n')],
        10: [('    ', 'p'), ('print', 'f'), ('(', 'p'), ('len', 'f'), ('(name))', 'p')],
        11: [('    return', 'k'), (' os.path.', 'p'), ('join', 'f'), ('(', 'p'), ('"a"', 's'), (', name)', 'p')],
        13: [('class', 'k'), (' ', 'p'), ('Greeter', 't'), (':', 'p')],
        14: [('    limit = ', 'p'), ('1.5', 'n')],
        15: [('    done = ', 'p'), ('True', 'c')],
    },
    'tokens.ts': {
        4: [('interface', 'k'), (' ', 'p'), ('Point', 't'), (' { x: ', 'p'), ('number', 't'), ('; label: ', 'p'),
            ('string', 't'), (' }', 'p')],
        5: [('const', 'k'), (' limit = ', 'p'), ('42', 'n'), (';', 'p')],
        6: [('export function', 'k'), (' ', 'p'), ('scale', 'f'), ('(p: ', 'p'), ('Point', 't'), (', k: ', 'p'),
            ('number', 't'), ('): ', 'p'), ('Point', 't'), (' {', 'p')],
        7: [('  if', 'k'), (' (k > ', 'p'), ('10', 'n'), (') { ', 'p'), ('return', 'k'), (' { x: p.x * ', 'p'), ('2', 'n'),
            (', label: ', 'p'), ('"big"', 's'), (' }; }', 'p')],
        8: [('  return', 'k'), (' { x: p.x * k, label: p.label.', 'p'), ('trim', 'f'), ('() };', 'p')],
        9: [('}', 'p')],
        10: [('let', 'k'), (' done: ', 'p'), ('boolean', 't'), (' = ', 'p'), ('true', 'c'), (';', 'p')],
    },
    'view.tsx': {
        4: [('type', 'k'), (' ', 'p'), ('Props', 't'), (' = { title: ', 'p'), ('string', 't'), (' };', 'p')],
        5: [('export function', 'k'), (' ', 'p'), ('Title', '?'), ('(props: ', 'p'), ('Props', 't'), (') {', 'p')],
        6: [('  const', 'k'), (' size = ', 'p'), ('3', 'n'), (';', 'p')],
        7: [('  return', 'k'), (' <h1 className=', '?'), ('"t"', 's'), ('>{', '?'), ('props.title', 'p'), ('}</h1>;', '?')],
    },
    'tokens.js': {
        4: [('const', 'k'), (' names = [', 'p'), ('"a"', 's'), (', ', 'p'), ('"b"', 's'), ('];', 'p')],
        5: [('function', 'k'), (' ', 'p'), ('count', 'f'), ('(list) {', 'p')],
        6: [('  let', 'k'), (' total = ', 'p'), ('0', 'n'), (';', 'p')],
        7: [('  for', 'k'), (' (', 'p'), ('const', 'k'), (' item ', 'p'), ('of', 'k'),
            (' list) { total += item.length; }', 'p')],
        8: [('  return', 'k'), (' total > ', 'p'), ('5', 'n'), (' ? ', 'p'), ('null', 'c'), (' : total;', 'p')],
        10: [('console.', 'p'), ('log', 'f'), ('(', 'p'), ('count', 'f'), ('(names));', 'p')],
    },
    'tokens.go': {
        4: [('package', 'k'), (' main', 'p')],
        6: [('import', 'k'), (' ', 'p'), ('"fmt"', 's')],
        8: [('type', 'k'), (' ', 'p'), ('Point', 't'), (' ', 'p'), ('struct', 'k'), ('{ X ', 'p'), ('int', 't'), (' }', 'p')],
        10: [('func', 'k'), (' ', 'p'), ('double', 'f'), ('(p ', 'p'), ('Point', 't'), (') ', 'p'), ('int', 't'), (' {', 'p')],
        11: [('\tif', 'k'), (' p.X > ', 'p'), ('9', 'n'), (' {', 'p')],
        12: [('\t\treturn', 'k'), (' ', 'p'), ('0', 'n')],
        14: [('\treturn', 'k'), (' p.X * ', 'p'), ('2', 'n')],
        17: [('func', 'k'), (' ', 'p'), ('main', 'f'), ('() { fmt.', 'p'), ('Println', 'f'), ('(', 'p'), ('double', 'f'),
             ('(', 'p'), ('Point', 't'), ('{X: ', 'p'), ('3', 'n'), ('}), ', 'p'), ('"ok"', 's'), (', ', 'p'), ('nil', 'c'),
             (') }', 'p')],
    },
    'tokens.c': {
        4: [('#include', 'k'), (' ', 'p'), ('<stdio.h>', 's')],
        5: [('#define', 'k'), (' ', 'p'), ('LIMIT', 'c'), (' 8', '?')],
        7: [('struct', 'k'), (' ', 'p'), ('point', 't'), (' { ', 'p'), ('int', 't'), (' x; };', 'p')],
        9: [('static', 'k'), (' ', 'p'), ('int', 't'), (' ', 'p'), ('twice', 'f'), ('(', 'p'), ('int', 't'), (' v) {', 'p')],
        10: [('    if', 'k'), (' (v > ', 'p'), ('LIMIT', 'c'), (') ', 'p'), ('return', 'k'), (' ', 'p'), ('0', 'n'), (';', 'p')],
        11: [('    return', 'k'), (' v * ', 'p'), ('2', 'n'), (';', 'p')],
        14: [('int', 't'), (' ', 'p'), ('main', 'f'), ('(', 'p'), ('void', 't'), (') { ', 'p'), ('printf', 'f'), ('(', 'p'),
             ('"%d\\n"', 's'), (', ', 'p'), ('twice', 'f'), ('(', 'p'), ('3', 'n'), (')); ', 'p'), ('return', 'k'), (' ', 'p'),
             ('0', 'n'), ('; }', 'p')],
    },
    'tokens.sh': {
        2: [(None, 'm')],
        4: [('set', 'f'), (' -eu', '?')],
        5: [('NAME=', 'p'), ('"world"', 's')],
        6: [('greet', 'f'), ('() {', 'p')],
        7: [('    local', '?'), (' who=', 'p'), ('$1', '?')],
        8: [('    if', 'k'), (' [ -n ', '?'), ('"', 's'), ('$who', '?'), ('"', 's'), (' ]; ', '?'), ('then', 'k'), (' ', 'p'),
            ('echo', 'f'), (' ', 'p'), ('"hello', 's'), (' ', 'p'), ('$who', '?'), ('"', 's'), ('; ', 'p'), ('fi', 'k')],
        10: [('for', 'k'), (' i ', 'p'), ('in', 'k'), (' 1 2 3; ', '?'), ('do', 'k'), (' ', 'p'), ('greet', 'f'), (' ', 'p'),
             ('"', 's'), ('$NAME', '?'), ('"', 's'), ('; ', 'p'), ('done', 'k')],
        11: [('exit', 'f'), (' 0', '?')],
    },
    'tokens.json': {
        2: [('  "name"', '?'), (': ', 'p'), ('"clanky"', 's'), (',', 'p')],
        4: [('  "version"', '?'), (': ', 'p'), ('15', 'n'), (',', 'p')],
        5: [('  "ratio"', '?'), (': ', 'p'), ('-0.5', 'n'), (',', 'p')],
        6: [('  "tags"', '?'), (': [', 'p'), ('"a"', 's'), (', ', 'p'), ('"b"', 's'), ('],', 'p')],
        7: [('  "ok"', '?'), (': ', 'p'), ('true', 'c'), (',', 'p')],
        8: [('  "none"', '?'), (': ', 'p'), ('null', 'c')],
        9: [('}', 'p')],
    },
    'tokens.toml': {
        5: [('name', '?'), (' = ', 'p'), ('"clanky"', 's')],
        6: [('version', '?'), (' = ', 'p'), ('"1.5.0"', 's')],
        7: [('jobs', '?'), (' = ', 'p'), ('8', 'n')],
        8: [('fast', '?'), (' = ', 'p'), ('true', 'c')],
        9: [('ratio', '?'), (' = ', 'p'), ('0.25', 'n')],
        10: [(None, 'm')],
    },
    'tokens.yaml': {
        4: [('name', '?'), (': ', 'p'), ('clanky', 's')],
        5: [('jobs', '?'), (': ', 'p'), ('8', 'n')],
        6: [('fast', '?'), (': ', 'p'), ('true', 'c')],
        7: [('ratio', '?'), (': ', 'p'), ('0.25', 'n')],
        9: [('  - ', 'p'), ('"quoted"', 's')],
        10: [('  - ', 'p'), ('null', 'c')],
        11: [(None, 'm')],
    },
    'notes.md': {
        2: [(None, 'p')],
        5: [('# ', '?'), ('Clanky notes', 'k')],
        7: [('Run ', 'p'), ('`', '?'), ('cargo build', 's'), ('`', '?'), (' first, then read ', 'p'),
            ('[the guide](https://example.invalid/guide)', '?'), ('.', 'p')],
        10: [('echo fenced', 's')],
        13: [(None, 'p')],
    },
}
if FAULT == 'table':
    RUST[13] = [('pub struct', 'k'), (' ', 'p'), ('Blink', 'k'), (' {', 'p')]


def cells(line):
    """Visual column (0-based) of each character, with tabs 4 wide as the editor draws them."""
    out, v = [], 0
    for ch in line:
        out.append(v)
        v += 4 - v % 4 if ch == '\t' else 1
    return out


def tokens(text_lines, table):
    """(line, col0, width, text, cls) for each judged token, cols in visual columns."""
    out = []
    for ln, pieces in table.items():
        line = text_lines[ln - 1]
        if pieces[0][0] is None:
            pieces = [(line, pieces[0][1])]
        joined = ''.join(t for t, _ in pieces)
        if joined != line:
            raise Fail(f'gate table: {ln}: pieces {joined!r} != fixture line {line!r}')
        vis, pos = cells(line), 0
        for text, cls in pieces:
            i = 0
            while i < len(text):
                if text[i] in ' \t':
                    i += 1
                    continue
                j = i
                while j < len(text) and text[j] not in ' \t':
                    j += 1
                if cls != '?':
                    a, b = pos + i, pos + j - 1
                    out.append((ln, vis[a], vis[b] - vis[a] + 1, text[i:j], cls))
                i = j
            pos += len(text)
    return out


# ---------------------------------------------------------------- window helpers

def shot(win, name):
    path = win.w / f'{name}.png'
    subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-f', 'x11grab', '-draw_mouse', '0', '-video_size', f'{W}x{H}',
                    '-i', win.env['DISPLAY'] + '+0,0', '-frames:v', '1', str(path)], env=win.env, check=True,
                   stdin=subprocess.DEVNULL)
    return np.asarray(Image.open(path).convert('RGB')).astype(np.int32)


def frames(win):
    """Wall-clock times of drawn frames (one "phases" line per frame)."""
    f = win.w / 'frames.csv'
    lines = f.read_text().splitlines() if f.exists() else []
    clock = next((int(l.split(',')[1]) for l in lines if l.startswith('clock,')), 0)
    return [(clock + int(l.split(',')[1])) / 1e6 for l in lines if l.startswith('phases,')]


def latencies(win, since):
    f = win.w / 'frames.csv'
    lines = f.read_text().splitlines() if f.exists() else []
    clock = next((int(l.split(',')[1]) for l in lines if l.startswith('clock,')), 0)
    return [int(p[4]) / 1000 for p in (l.split(',') for l in lines if l.startswith('latency,'))
            if (clock + int(p[2])) / 1e6 >= since]


def window(name, **cfg):
    win = Window(name, cursor_blink='off', **cfg)
    win.x('windowsize', '--sync', win.wid, str(W), str(H))
    time.sleep(1)
    win.key('ctrl+shift+b')
    time.sleep(1)
    return win


def solid_blocks(im):
    """Uniform rectangles 4-40 px wide and 6-60 px tall, bounded on all four sides by other colours."""
    change = (im[:, 1:] != im[:, :-1]).any(axis=2)
    active, rects = {}, []
    for y in range(H + 1):
        keys = set()
        if y < H:
            edges = np.concatenate(([0], np.nonzero(change[y])[0] + 1, [W]))
            for x0, x1 in zip(edges[:-1], edges[1:]):
                if 4 <= x1 - x0 <= 40 and 0 < x0 and x1 < W:
                    keys.add((int(x0), int(x1), tuple(int(v) for v in im[y, x0])))
        for key in list(active):
            if key not in keys:
                top = active.pop(key)
                x0, x1, c = key
                if 6 <= y - top <= 60 and top > 0 and y < H and not (im[top - 1, x0:x1] == c).all(axis=1).all() \
                        and not (im[y, x0:x1] == c).all(axis=1).all():
                    rects.append((x0, top, x1 - x0, y - top, c))
        for key in keys:
            active.setdefault(key, y)
    return rects


def geometry(im):
    """Cell grid from the marker blocks (line 1 cols 7-8, line 3 cols 27-28; the two blocks of a pair may be drawn
    apart or as one rectangle). Returns (x0, y0, cw, ch), the pixel box of line 1 column 1, or None."""
    rects = solid_blocks(im)
    found = []
    for a in rects:
        for c in rects:
            if c[4] != a[4] or c[2:4] != a[2:4] or c[1] <= a[1] or c[0] <= a[0]:
                continue
            cw, ch = (c[0] - a[0]) / 20, (c[1] - a[1]) / 2
            if not (4 <= cw <= 30 and 1.3 * cw <= ch <= 4 * cw):
                continue
            twin = lambda q: any(r[4] == q[4] and r[1] == q[1] and r[2:4] == q[2:4] and abs(r[0] - q[0] - cw) <= 1
                                 for r in rects)
            twin = twin(a) and twin(c)
            if twin or a[2] >= 1.6 * cw:
                width = a[2] if twin else a[2] / 2
                found.append((a[0] - (cw - width) / 2 - 6 * cw, a[1] - (ch - a[3]) / 2, cw, ch))
    return found[0] if len(found) == 1 else None


def open_file(win, path, label):
    win.key('ctrl+shift+o')
    time.sleep(.5)
    win.type(str(path))
    win.key('Return')
    time.sleep(1.5)
    geo = until(lambda: geometry(shot(win, label)), f'setup: {path.name} opens in an editor pane and its four marker '
                                                    f'blocks are found', 8)
    return geo


def close_file(win, label):
    win.key('ctrl+shift+w')
    time.sleep(1)
    require(geometry(shot(win, label + '-closed')) is None, f'setup: Ctrl+Shift+W closes the editor pane ({label})')


# ---------------------------------------------------------------- colour judging

def background(im, geo, nlines):
    x0, y0, cw, ch = geo
    reg = im[int(y0):int(y0 + nlines * ch), int(x0):int(x0 + 90 * cw)].reshape(-1, 3)
    vals, counts = np.unique(reg, axis=0, return_counts=True)
    return vals[counts.argmax()]


def judge(im, geo, bg, line, col0, width, palette, depth_of=None):
    """(class, residual, n) of the ink in the cells, or (None, None, n) when there is too little ink. With depth_of
    (a colour) it returns (class, residual, n, ink, fit): the 99th percentile of the pixels' a and the weighted
    residual when the ink is fitted to that colour."""
    x0, y0, cw, ch = geo
    xa, xb = int(round(x0 + col0 * cw)), int(round(x0 + (col0 + width) * cw))
    ya, yb = int(round(y0 + (line - 1) * ch)), int(round(y0 + line * ch))
    px = im[ya:yb, xa:xb].reshape(-1, 3).astype(float)
    bgf = bg.astype(float)
    dist = np.abs(px - bgf).max(axis=1)
    px, w = px[dist >= 12], dist[dist >= 12]
    if len(px) < 3:
        return (None, None, len(px)) + ((None, None) if depth_of is not None else ())
    best = None
    for cls, c in palette.items():
        d = np.array(c, float) - bgf
        a = np.clip(((px - bgf) @ d) / (d @ d), 0, 1)
        res = np.abs(px - (bgf + a[:, None] * d)).max(axis=1)
        score = float((res * w).sum() / w.sum())
        if best is None or score < best[1]:
            best = (cls, score)
    if depth_of is not None:
        d = np.array(depth_of, float) - bgf
        a = np.clip(((px - bgf) @ d) / (d @ d), 0, 1)
        res = np.abs(px - (bgf + a[:, None] * d)).max(axis=1)
        return best[0], best[1], len(px), float(np.percentile(a, 99)), float((res * w).sum() / w.sum())
    return best[0], best[1], len(px)


def hexc(c):
    return '#%02x%02x%02x' % tuple(c)


def check_tokens(im, geo, file_label, text_lines, table, palette, theme):
    bg = background(im, geo, len(text_lines))
    toks = tokens(text_lines, table)
    old = theme_colours(theme, old=True)
    judged, comment_ink = 0, {}
    for ln, col0, width, text, cls in toks:
        got, score, n, ink, _ = judge(im, geo, bg, ln, col0, width, palette, depth_of=palette[cls])
        if got is None:
            require(n > 0 or cls == 'p', f'{file_label}:{ln}:{col0 + 1} "{text}" not drawn (expected {NAMES[cls]})')
            continue
        judged += 1
        # Diagnosis only, never decides: how well the ink fits the colour the class had before R2.
        why = ''
        if cls in 'mn' and old[cls] != palette[cls]:
            _, _, _, oink, ofit = judge(im, geo, bg, ln, col0, width, palette, depth_of=old[cls])
            why = f'; the old {NAMES[cls]} colour {hexc(old[cls])} fits with residual {ofit:.1f}, ink {oink:.2f}'
        require(got == cls and score <= 3, f'{file_label}:{ln}:{col0 + 1} "{text}" expected {NAMES[cls]} '
                f'{hexc(palette[cls])} ({theme}), drawn as {NAMES[got]} {hexc(palette[got])} (residual {score:.1f}, '
                f'ink {ink:.2f} of expected){why}')
        if cls == 'm' and ink > comment_ink.get(ln, (-1,))[0]:
            comment_ink[ln] = (ink, why)
    for ln, (ink, why) in comment_ink.items():
        require(ink >= .75, f'{file_label}:{ln} comment line expected in {hexc(palette["m"])} ({theme}) at full ink, '
                f'drawn fainter: its ink reaches {ink:.2f} of it (want >= 0.75){why}')
    require(judged >= .9 * len(toks), f'{file_label}: {judged} of {len(toks)} tokens had enough ink to judge')
    return judged


def class_map(im, geo, text_lines, palette):
    """Colour class of every non-space cell, except line 1 (the caret's line)."""
    bg = background(im, geo, len(text_lines))
    out = {}
    for ln, line in enumerate(text_lines, 1):
        if ln == 1:
            continue
        for ch, v in zip(line, cells(line)):
            if ch not in ' \t':
                got, _, _ = judge(im, geo, bg, ln, v, 1, palette)
                if got:
                    out[(ln, v + 1)] = got
    return out


# ---------------------------------------------------------------- stories

def fixture_story(win, palette, theme, files):
    for name in files:
        path = FIX / name
        judged_as = name
        if FAULT == 'language' and name == 'tokens.py':
            copy = ROOT / 'lang' / 'tokens.py.txt'
            copy.parent.mkdir(exist_ok=True)
            shutil.copy(path, copy)
            path = copy
        lines = (FIX / judged_as).read_text().split('\n')
        geo = open_file(win, path, f'{theme}-{name}')
        im = shot(win, f'{theme}-{name}')
        n = check_tokens(im, geo, name, lines, EXPECT[judged_as], palette, theme)
        if name == 'cursor_blink.rs':
            italic_check(im, geo, background(im, geo, len(lines)))
        close_file(win, f'{theme}-{name}')
        print(f'ok: {theme} {name} ({n} tokens)')


def slant(im, geo, bg, line, col0):
    """Top-half minus bottom-half ink centroid x of one glyph cell, and the vertical distance of the centroids."""
    x0, y0, cw, ch = geo
    xa, xb = int(round(x0 + col0 * cw)), int(round(x0 + (col0 + 1) * cw))
    ya, yb = int(round(y0 + (line - 1) * ch)), int(round(y0 + line * ch))
    m = int(cw / 2)
    cell = np.abs(im[ya:yb, max(0, xa - m):xb + m] - bg).max(axis=2).astype(float)
    rows = np.nonzero(cell.max(axis=1) >= 12)[0]
    require(len(rows) >= 4, f'C3 glyph at {line}:{col0 + 1} has ink')
    mid = (rows[0] + rows[-1] + 1) / 2
    xs = np.arange(cell.shape[1])
    ys = np.arange(cell.shape[0])
    top, bot = cell[ys < mid], cell[ys >= mid]
    cx = lambda part: (part.sum(axis=0) * xs).sum() / part.sum()
    cy = lambda part, off: (part.sum(axis=1) * (np.arange(part.shape[0]) + off)).sum() / part.sum()
    return cx(top) - cx(bot), cy(bot, int(math.ceil(mid))) - cy(top, 0)


def italic_check(im, geo, bg):
    # line 8 "// | | | | | |": | at columns 4, 6, ... 14; line 10 "fn tilt(l: u8) -> u8 { l | l | l | l }": the three
    # |'s after "{" (columns 26, 30, 34), each with a space on both sides like the comment's
    plain = [(10, 25), (10, 29), (10, 33)]
    comment = plain if FAULT == 'italic' else [(8, col) for col in range(3, 14, 2)]
    c = [slant(im, geo, bg, ln, col) for ln, col in comment]
    p = [slant(im, geo, bg, ln, col) for ln, col in plain]
    lean = sum(s for s, _ in c) / len(c) - sum(s for s, _ in p) / len(p)
    dy = sum(d for _, d in c + p) / len(c + p)
    want = .5 * dy * math.tan(math.radians(SLANT_DEG))
    require(lean >= want, f'C3 comment glyphs are italic: comment "|" leans {lean:.2f} px more than plain "|" '
                          f'(want >= {want:.2f} px)')


def edit_story(win, palette):
    src = FIX / 'cursor_blink.rs'
    work = ROOT / 'edit'
    work.mkdir(exist_ok=True)
    path = work / 'cursor_blink.rs'
    shutil.copy(src, path)
    lines = src.read_text().split('\n')
    geo = open_file(win, path, 'edit-open')

    def park():
        # The block caret covers its cell with the cursor colour and redraws the glyph on it in the background
        # colour, so the cell under it can never read as its token's class. Park it at line 1 column 1 (a marker
        # line, never judged) before a screenshot whose judged lines hold the caret, as in step f.
        win.x('key', '--delay', '40', *['Up'] * 12, 'Home')
        time.sleep(.5)

    def look(label, model, checks):
        im = shot(win, label)
        for ln, pieces in checks.items():
            check_tokens(im, geo, f'{label} cursor_blink.rs', model, {ln: pieces}, palette, 'stone-dark')
        return im

    # a. comment out line 13
    win.key(*['Down'] * 12, 'Home')
    if FAULT != 'edit':
        win.type('// ')
    model = list(lines)
    model[12] = '// ' + model[12]
    park()
    look('edit-a', model, {13: [(None, 'm')], 18: RUST[18]})
    # b. remove it again (back to 13:4, after the "// ")
    win.x('key', '--delay', '40', *['Down'] * 12, 'Home', 'Right', 'Right', 'Right')
    win.key('BackSpace', 'BackSpace', 'BackSpace')
    model[12] = lines[12]
    park()
    look('edit-b', model, {13: RUST[13]})
    # c. a new line 11
    win.x('key', '--delay', '40', *['Down'] * 10)
    win.key('End')
    win.type('const NN: u8 = 7;')
    win.key('Return')
    model[10] = 'const NN: u8 = 7;'
    model.insert(11, '')
    time.sleep(.5)
    look('edit-c', model, {11: [('const', 'k'), (' ', 'p'), ('NN', 'c'), (': ', 'p'), ('u8', 't'), (' = ', 'p'),
                                ('7', 'n'), (';', 'p')]})
    im = shot(win, 'edit-c-mockup')
    for ln in range(12, 27):
        if ln in RUST:
            check_tokens(im, geo, f'edit-c (mockup line {ln} now at {ln + 1})', model, {ln + 1: RUST[ln]},
                         palette, 'stone-dark')
    # d. block comment over lines 6-7
    win.key(*['Up'] * 6, 'Home')
    win.type('/*')
    win.key('Down', 'End')
    win.type('*/')
    last_key = time.time()
    model[5] = '/*' + model[5]
    model[6] = model[6] + '*/'
    time.sleep(.5)
    im = look('edit-d', model, {6: [(None, 'm')], 7: [(None, 'm')], 10: RUST[10]})
    for ln in range(12, 27):
        if ln in RUST:
            check_tokens(im, geo, f'edit-d (mockup line {ln} now at {ln + 1})', model, {ln + 1: RUST[ln]},
                         palette, 'stone-dark')
    # I1: nothing redraws once nothing changes
    if FAULT == 'idle':
        time.sleep(max(0, last_key + 2.5 - time.time()))
        win.key('Right')
    time.sleep(max(0, last_key + 4.5 - time.time()))
    late = [t for t in frames(win) if last_key + 1 < t < last_key + 4]
    require(not late, f'I1 0 frames 1-4 s after the last key: {len(late)}')
    print('ok: idle')
    # e. save and compare the bytes on disk
    win.key('ctrl+s')
    time.sleep(1)
    want = '\n'.join(model)
    got = path.read_text()
    require(got == want, f'E1e saved file equals the fixture with the edits ({len(got)} vs {len(want)} bytes)')
    # f. incremental colours equal a fresh parse
    win.x('key', '--delay', '40', *['Up'] * 30, 'Home')
    time.sleep(.5)
    edited = class_map(shot(win, 'edit-f'), geo, model, palette)
    close_file(win, 'edit-f')
    geo2 = open_file(win, path, 'edit-fresh')
    fresh = class_map(shot(win, 'edit-fresh'), geo2, model, palette)
    diff = sorted(k for k in set(edited) | set(fresh) if edited.get(k) != fresh.get(k))
    require(not diff, f'E1f edited pane colours equal a fresh pane on the saved file: {len(diff)} cells differ, first '
                      f'{[(k, edited.get(k), fresh.get(k)) for k in diff[:3]]}')
    require(sum(1 for v in fresh.values() if v != 'p') >= 40, 'E1f the fresh pane is coloured (40+ coloured cells)')
    close_file(win, 'edit-fresh')
    print('ok: incremental edits')


def perf_story(win):
    work = ROOT / 'perf'
    work.mkdir(exist_ok=True)
    text = subprocess.run(['git', '-C', str(REPO), 'show', f'{PERF_COMMIT}:crates/clanky/src/studio.rs'],
                          capture_output=True, check=True).stdout
    med = {}
    for ext in ('txt', 'rs'):
        path = work / f'studio.{ext}'
        path.write_bytes(text)
        open_file_plain(win, path, f'perf-{ext}')
        win.x('key', '--delay', '40', *['Down'] * 39, 'End')
        time.sleep(1)
        start = time.time()
        win.x('type', '--delay', '150', 'x' * 40)
        time.sleep(1.5)
        lat = sorted(latencies(win, start))
        require(len(lat) >= 35, f'P1 frame trace has key latencies for .{ext}: {len(lat)}')
        med[ext] = lat[len(lat) // 2]
        win.key('ctrl+s')  # the copy is scratch; saving leaves the pane clean so Ctrl+Shift+W closes it
        win.key('ctrl+shift+w')
        time.sleep(1)
    extra = med['rs'] - med['txt']
    require(extra <= 5, f'P1 keystroke median latency .rs {med["rs"]:.1f} ms vs .txt {med["txt"]:.1f} ms: '
                        f'{extra:.1f} ms extra (want <= 5)')
    print(f'ok: keystroke median .rs {med["rs"]:.1f} ms, .txt {med["txt"]:.1f} ms')


def open_file_plain(win, path, label):
    """Open a file that has no markers (the perf input)."""
    win.key('ctrl+shift+o')
    time.sleep(.5)
    win.type(str(path))
    win.key('Return')
    time.sleep(2)


def main():
    for tool in ('Xvfb', 'xdotool', 'ffmpeg'):
        require(shutil.which(tool), f'tool {tool} installed')
    shutil.rmtree(ROOT, ignore_errors=True)
    ROOT.mkdir(parents=True, mode=0o700)
    for name, (m, n) in HAND.items():
        pal = theme_colours(name)
        require((hexc(pal['m']), hexc(pal['n'])) == (m, n), f'gate table: {name} comment/number from the formulas '
                f'{hexc(pal["m"])} {hexc(pal["n"])} equal the hand-worked {m} {n}')
    dark = theme_colours('stone-light' if FAULT == 'theme' else 'stone-dark')
    if FAULT == 'oldmap':
        dark['m'] = theme_colours('stone-dark', old=True)['m']
    light = theme_colours('stone-light')
    order = ['cursor_blink.rs', 'tokens.py', 'tokens.ts', 'view.tsx', 'tokens.js', 'tokens.go', 'tokens.c',
             'tokens.sh', 'tokens.json', 'tokens.toml', 'tokens.yaml', 'notes.md']
    if FAULT == 'language':
        order = ['tokens.py']
    win = window('dark', theme='dark', theme_dark='stone-dark', terminal_colors=RILEY)
    try:
        if FAULT in (None, 'table', 'theme', 'italic', 'language', 'oldmap'):
            fixture_story(win, dark, 'stone-dark', order)
        if FAULT in (None, 'edit', 'idle'):
            edit_story(win, dark)
        if FAULT is None:
            perf_story(win)
    finally:
        win.close()
    if FAULT is None:
        win = window('light', theme='light', theme_light='stone-light')
        try:
            fixture_story(win, light, 'stone-light', ['cursor_blink.rs', 'tokens.py'])
        finally:
            win.close()
        shutil.rmtree(ROOT, ignore_errors=True)
    print('PASS: Q3 syntax')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: Q3 {e}')
        sys.exit(1)
    except Exception as e:  # a crash still reports FAIL, never PASS
        print(f'FAIL: Q3 unexpected {type(e).__name__}: {e}')
        sys.exit(1)
