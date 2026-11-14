#!/usr/bin/env python3
"""Build assets/terminal-schemes/schemes.json and SOURCES.md from the source files.

The values are copied from permissively licensed sources checked out under /srv/devops/build/ref (pass another
folder as the first argument): the iTerm2-Color-Schemes collection (each scheme's own licence from its
licenses.yml), the Gogh collection (MIT OR Apache-2.0), and a few projects' own files at a pinned commit
(Windows Terminal, Alacritty, WezTerm, Dracula, Solarized). Only colour values are copied, never code.

A scheme without a selection colour gets one a quarter of the way from its background to its foreground; one
without a cursor colour draws the cursor in its foreground.
"""
import json, plistlib, re, sys
from pathlib import Path
from urllib.parse import quote

REF = Path(sys.argv[1] if len(sys.argv) > 1 else '/srv/devops/build/ref')
OUT = Path(__file__).resolve().parents[1] / 'assets/terminal-schemes'
COLLECTION = ('mbadolato/iTerm2-Color-Schemes', '95705098082c5d07fad152c31f020d50d38e10e8')
GOGH = ('Gogh-Co/Gogh', 'fd715c810bb0ad35f630bd73fb71f5b5a1311bd6')
PRIMARY = {
    'terminal': ('microsoft/terminal', 'f6de9e0618015def1c9dda5b82f9d3abf03eb799', 'MIT'),
    'alacritty': ('alacritty/alacritty', 'aa5d942f46c40184a3f214830ff5f7989f4733a1', 'Apache-2.0'),
    'wezterm': ('wezterm/wezterm', 'e1eae81a56be8b5487dc77a29616afa568fb57b7', 'MIT'),
    'dracula': ('dracula/iterm', 'e5749bcd07764ad6e154038892b118ea45f88cc0', 'MIT'),
    'solarized': ('altercation/solarized', '62f656a02f93c5190a8753159e34b385588d5ff3', 'MIT'),
}

# (id, name shown, kind, source name, note). Terminal defaults first, then themes by name.
SCHEMES = [
    ('xterm', 'xterm', 'gogh', 'Xterm', "xterm's 16 colours (XTerm-col.ad)"),
    ('gnome-terminal', 'GNOME Terminal', 'gogh', 'Gnome Terminal', 'the GNOME palette of GNOME Terminal and Ptyxis'),
    ('konsole', 'Konsole Breeze', 'gogh', 'Breeze', "Konsole's default Breeze scheme"),
    ('campbell', 'Campbell', 'wt', 'Campbell', "Windows Terminal's default"),
    ('macos-terminal', 'macOS Terminal', 'gogh', 'Terminal App', "Terminal.app's default colours"),
    ('iterm2', 'iTerm2', 'gogh', 'iTerm2 Default', "iTerm2's default"),
    ('alacritty', 'Alacritty', 'alacritty', 'alacritty/src/config/color.rs', "Alacritty's default"),
    ('kitty', 'Kitty', 'gogh', 'Kitty Default', "kitty's default"),
    ('wezterm', 'WezTerm', 'wezterm', 'term/src/color.rs', "WezTerm's default"),
    ('ghostty', 'Ghostty', 'collection', 'Ghostty Default Style Dark', "Ghostty's default"),
    ('ayu-mirage', 'Ayu Mirage', 'collection', 'Ayu Mirage', ''),
    ('catppuccin-latte', 'Catppuccin Latte', 'collection', 'Catppuccin Latte', ''),
    ('catppuccin-mocha', 'Catppuccin Mocha', 'collection', 'Catppuccin Mocha', ''),
    ('dracula', 'Dracula', 'dracula', 'Dracula.itermcolors', ''),
    ('everforest-dark', 'Everforest Dark', 'collection', 'Everforest Dark Hard', 'the hard variant'),
    ('everforest-light', 'Everforest Light', 'collection', 'Everforest Light Med', 'the medium variant'),
    ('github-dark', 'GitHub Dark', 'collection', 'GitHub Dark Default', ''),
    ('github-light', 'GitHub Light', 'collection', 'GitHub Light Default', ''),
    ('gruvbox-dark', 'Gruvbox Dark', 'collection', 'Gruvbox Dark', ''),
    ('gruvbox-light', 'Gruvbox Light', 'collection', 'Gruvbox Light', ''),
    ('kanagawa', 'Kanagawa', 'collection', 'Kanagawa Wave', 'Kanagawa Wave, the default'),
    ('kanagawa-dragon', 'Kanagawa Dragon', 'collection', 'Kanagawa Dragon', ''),
    ('material', 'Material', 'collection', 'Material', ''),
    ('monokai', 'Monokai', 'gogh', 'Monokai', ''),
    ('night-owl', 'Night Owl', 'gogh', 'Night Owl', ''),
    ('nord', 'Nord', 'collection', 'Nord', ''),
    ('oceanic-next', 'Oceanic Next', 'gogh', 'Oceanic Next', ''),
    ('one-dark', 'One Dark', 'collection', 'Atom One Dark', "Atom's One Dark"),
    ('one-light', 'One Light', 'collection', 'Atom One Light', "Atom's One Light"),
    ('rose-pine', 'Rosé Pine', 'collection', 'Rose Pine', ''),
    ('rose-pine-moon', 'Rosé Pine Moon', 'collection', 'Rose Pine Moon', ''),
    ('rose-pine-dawn', 'Rosé Pine Dawn', 'collection', 'Rose Pine Dawn', ''),
    ('solarized-dark', 'Solarized Dark', 'solarized', 'iterm2-colors-solarized/Solarized Dark.itermcolors', ''),
    ('solarized-light', 'Solarized Light', 'solarized', 'iterm2-colors-solarized/Solarized Light.itermcolors', ''),
    ('tokyo-night', 'Tokyo Night', 'collection', 'TokyoNight Night', ''),
    ('tokyo-night-storm', 'Tokyo Night Storm', 'collection', 'TokyoNight Storm', ''),
    ('tokyo-night-moon', 'Tokyo Night Moon', 'collection', 'TokyoNight Moon', ''),
    ('tokyo-night-day', 'Tokyo Night Day', 'collection', 'TokyoNight Day', ''),
    ('tomorrow-night', 'Tomorrow Night', 'collection', 'Tomorrow Night', ''),
    ('zenburned', 'Zenburned', 'collection', 'Zenburned', "zenbones' Zenburn"),
]


def hexs(c):
    return '#%02x%02x%02x' % tuple(c)


def h(text):
    text = text.strip().strip('\'"').lower()
    text = text[2:] if text.startswith('0x') else text.lstrip('#')
    assert re.fullmatch(r'[0-9a-f]{6}', text), text
    return tuple(int(text[i:i + 2], 16) for i in (0, 2, 4))


def iterm(path):
    d = plistlib.loads(Path(path).read_bytes())
    c = lambda k: tuple(round(d[k][n] * 255) for n in ('Red Component', 'Green Component', 'Blue Component')) if k in d else None
    return dict(background=c('Background Color'), foreground=c('Foreground Color'), cursor=c('Cursor Color'),
                selection=c('Selection Color'), ansi=[c(f'Ansi {i} Color') for i in range(16)])


def gogh(path):
    kv = {}
    for line in Path(path).read_text().splitlines():
        m = re.match(r"^\s*([\w]+):\s*'([^']*)'", line)
        if m:
            kv[m.group(1)] = m.group(2)
    g = lambda k: h(kv[k]) if kv.get(k) else None
    return dict(background=g('background'), foreground=g('foreground'), cursor=g('cursor'), selection=None,
                ansi=[g('color_%02d' % i) for i in range(1, 17)])


def windows_terminal(name):
    text = (REF / 'primary/terminal/defaults.json').read_text()
    text = re.sub(r',(\s*[}\]])', r'\1', re.sub(r'^\s*//.*$', '', text, flags=re.M))
    s = next(s for s in json.loads(text)['schemes'] if s['name'] == name)
    names = ['black', 'red', 'green', 'yellow', 'blue', 'purple', 'cyan', 'white']
    ansi = [h(s[n]) for n in names] + [h(s['bright' + n.title()]) for n in names]
    return dict(background=h(s['background']), foreground=h(s['foreground']), cursor=h(s['cursorColor']),
                selection=h(s['selectionBackground']) if 'selectionBackground' in s else None, ansi=ansi)


def rust_rgbs(text, start, count):
    """`count` Rgb::new(0x.., 0x.., 0x..) triples from the first one after `start`."""
    found = re.findall(r'new(?:_8bpc)?\(\s*0x([0-9a-f]{2}),\s*0x([0-9a-f]{2}),\s*0x([0-9a-f]{2})\)', text[text.index(start):])
    return [tuple(int(x, 16) for x in t) for t in found[:count]]


def alacritty():
    text = (REF / 'primary/alacritty/color.rs').read_text()
    bg, fg = rust_rgbs(text, 'impl Default for PrimaryColors', 2)
    normal = rust_rgbs(text, 'impl Default for NormalColors', 8)
    bright = rust_rgbs(text, 'impl Default for BrightColors', 8)
    return dict(background=bg, foreground=fg, cursor=None, selection=None, ansi=normal + bright)


def wezterm():
    text = (REF / 'primary/wezterm/color.rs').read_text()
    ansi = rust_rgbs(text, 'let ansi: [SrgbaTuple; 16]', 16)
    cursor = rust_rgbs(text, 'let cursor_bg', 1)[0]
    # foreground is colors[249], Grey70 of the 24-step grey ramp (0x08 + 10 * 17); selection_bg is
    # (0.5, 0.4, 0.6) at alpha 0.5 over the black background.
    assert 'colors[249]; // Grey70' in text and 'SrgbaTuple(0.5, 0.4, 0.6, 0.5)' in text
    return dict(background=ansi[0], foreground=(0xb2,) * 3, cursor=cursor,
                selection=tuple(round(v * 0.5 * 255) for v in (0.5, 0.4, 0.6)), ansi=ansi)


def finish(s):
    assert len(s['ansi']) == 16 and all(s['ansi']) and s['background'] and s['foreground'], s
    s['cursor'] = s['cursor'] or s['foreground']
    s['selection'] = s['selection'] or tuple(round(b + (f - b) * 0.25) for b, f in zip(s['background'], s['foreground']))
    return {k: hexs(s[k]) for k in ('background', 'foreground', 'cursor', 'selection')} | {'ansi': [hexs(c) for c in s['ansi']]}


def main():
    import yaml
    licences = yaml.safe_load((REF / 'iTerm2-Color-Schemes/licenses.yml').read_text())
    out, rows = [], []
    for sid, name, kind, src, note in SCHEMES:
        if kind == 'collection':
            colors = iterm(REF / f'iTerm2-Color-Schemes/schemes/{src}.itermcolors')
            repo, commit = COLLECTION
            url = f'https://github.com/{repo}/blob/{commit}/schemes/{quote(src)}.itermcolors'
            licence = licences[src]['license']
        elif kind == 'gogh':
            colors = gogh(REF / f'gogh/themes/{src}.yml')
            repo, commit = GOGH
            url = f'https://github.com/{repo}/blob/{commit}/themes/{quote(src)}.yml'
            licence = 'MIT OR Apache-2.0'
        else:
            key = {'wt': 'terminal'}.get(kind, kind)
            repo, commit, licence = PRIMARY[key]
            path = {'wt': 'src/cascadia/TerminalSettingsModel/defaults.json'}.get(kind, src)
            url = f'https://github.com/{repo}/blob/{commit}/{quote(path)}'
            colors = {'wt': lambda: windows_terminal(src), 'alacritty': alacritty, 'wezterm': wezterm,
                      'dracula': lambda: iterm(REF / 'primary/dracula/Dracula.itermcolors'),
                      'solarized': lambda: iterm(REF / 'primary/solarized' / Path(src).name)}[kind]()
        assert licence in ('MIT', 'MIT OR Apache-2.0', 'Apache-2.0', 'ISC', 'CC0-1.0', 'BSD-2-Clause', 'BSD-3-Clause'), (name, licence)
        out.append({'id': sid, 'name': name} | finish(colors))
        rows.append(f'| {name} | {url} | {commit} | {licence} | {note} |')
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / 'schemes.json').write_text(json.dumps(out, indent=1, ensure_ascii=False) + '\n')
    (OUT / 'SOURCES.md').write_text(
        '# Where the built-in terminal colour schemes come from\n\n'
        'Each scheme\'s colour values (background, foreground, cursor, selection and the 16 ANSI colours) are copied\n'
        'from the file below, at the commit given, under that file\'s licence. No code is copied. A scheme whose source\n'
        'gives no selection colour uses the colour a quarter of the way from its background to its foreground; one with\n'
        'no cursor colour draws the cursor in its foreground. `scripts/terminal-schemes.py` rebuilds `schemes.json` and\n'
        'this table from the source files.\n\n'
        '| Scheme | Source | Commit | Licence | Notes |\n|---|---|---|---|---|\n' + '\n'.join(rows) + '\n')
    print(f'{len(out)} schemes')


if __name__ == '__main__':
    main()
