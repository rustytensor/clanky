#!/usr/bin/env python3
"""Compare the independently specified M12 geometry, pixels and idle observations.

This is a partial gate: interaction, theme loading and Post-AI API checks still
need the app interfaces. It must never be presented as the complete M12 gate.
JSON schema: layout {inset, a, b, pty_a, pty_b}; hidden {inset, a};
drag {a, b, restored_a, restored_b}; post_ai {a}; each pane has rect,
origin (where specified), cols and rows. pixels {dark, light, bedazzled} maps
"x,y" to premultiplied RGBA. ui_text_px is the scene's full UI text-run size
list. idle {no_input_frames, key_echo_frames} comes from the real redraw path.
"""
import json
import math
import sys


class Mismatch(Exception):
    pass


def exact(value, expected, label):
    if isinstance(expected, list):
        if not isinstance(value, list) or len(value) != len(expected):
            raise Mismatch(f'{label}: expected {expected!r}, observed {value!r}')
        for index, (actual, wanted) in enumerate(zip(value, expected)):
            exact(actual, wanted, f'{label}[{index}]')
    elif type(value) not in (int, float) or not math.isfinite(value) or value != expected:
        raise Mismatch(f'{label}: expected {expected!r}, observed {value!r}')


def pixels(actual, expected, name):
    for xy, wanted in expected.items():
        value = actual[xy]
        if not (isinstance(value, list) and len(value) == 4 and
                all(type(v) is int and 0 <= v <= 255 for v in value) and
                all(abs(a - b) <= 3 for a, b in zip(value, wanted))):
            raise Mismatch(f'{name} {xy}: expected {wanted!r} ±3, observed {value!r}')


def compare(data):
    layout = data['layout']
    exact(layout['inset'], [256, 8, 976, 744], 'inset')
    for key, rect, origin in [('a', [268, 76, 672, 326], [280, 126]),
                              ('b', [268, 414, 672, 326], [280, 464])]:
        pane = layout[key]
        exact(pane['rect'], rect, f'pane {key}')
        exact(pane['origin'], origin, f'grid {key} origin')
        exact([pane['cols'], pane['rows']], [81, 13], f'grid {key} size')
        exact(layout['pty_' + key], [13, 81], f'PTY {key} size')
    hidden = data['hidden']
    exact(hidden['inset'], [8, 8, 1224, 744], 'hidden inset')
    exact(hidden['a']['rect'], [20, 76, 1200, 326], 'hidden pane')
    exact([hidden['a']['cols'], hidden['a']['rows']], [147, 13], 'hidden grid size')
    drag = data['drag']
    for key, rect, rows in [('a', [268, 76, 672, 218], 7),
                            ('b', [268, 306, 672, 434], 18)]:
        exact(drag[key]['rect'], rect, f'drag pane {key}')
        exact(drag[key]['rows'], rows, f'drag rows {key}')
        exact(drag['restored_' + key]['rect'], rect, f'restored pane {key}')
    pane = data['post_ai']['a']
    exact(pane['rect'], [268, 76, 952, 326], 'Post-AI pane')
    exact(pane['cols'], 116, 'Post-AI columns')
    coords = ['128,700', '257,9', '264,16', '600,63', '939,250', '939,600', '930,390', '968,200']
    dark = [[20,18,16,255], [20,18,16,255], [26,24,22,255], [44,41,38,255],
            [44,41,38,255], [138,129,120,255], [26,24,22,255], [32,29,26,255]]
    light = [[243,240,235,255], [243,240,235,255], [251,250,248,255], [231,225,217,255],
             [231,225,217,255], [163,154,143,255], [251,250,248,255], [255,255,255,255]]
    pixels(data['pixels']['dark'], dict(zip(coords, dark)), 'Stone dark')
    pixels(data['pixels']['light'], dict(zip(coords, light)), 'Stone light')
    pixels(data['pixels']['bedazzled'], {'128,20':[122,59,237,255], '128,740':[39,98,235,255]}, 'AI Bedazzled')
    sizes = data['ui_text_px']
    if not isinstance(sizes, list) or not sizes or not all(type(x) in (int, float) and math.isfinite(x) and x >= 12 for x in sizes):
        raise Mismatch('UI text runs: expected nonempty list of finite sizes >= 12 px')
    idle = data['idle']
    exact(idle['no_input_frames'], 0, 'idle frames')
    frames = idle['key_echo_frames']
    if type(frames) is not int or not 1 <= frames <= 2:
        raise Mismatch(f'key echo frames: expected 1..2, observed {frames!r}')


def main():
    try:
        compare(json.load(sys.stdin))
    except (Mismatch, KeyError, TypeError, ValueError) as error:
        print(f'FAIL: M12 partial comparisons: {error}', file=sys.stderr)
        return 1
    print('PASS: M12 partial comparisons only (clicks, themes and Post-AI API not checked)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
