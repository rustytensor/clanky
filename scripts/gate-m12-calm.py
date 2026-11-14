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
import argparse
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


def compare(data, scene_only=False, no_idle=False):
    layout = data['layout']
    # Quiet layout (design 2026-11-13, mockup Marc approved, Y1 of round 2), 1240 x 760, 8 x 20
    # cells. [M] mockup CSS: .win rows 38px 1fr 26px (title bar, status line), .body columns 236px 1fr (tools
    # panel), .blk padding 18px left and right, .blocks padding-top 6px, 1 px hairlines between regions; [Q1] the
    # Q1 gate: grid x = 236 + 18, columns floor((W - 236 - 36) / cell), no pane header; [N] notes clanky-ui.md 5.2
    # (worked out from the mockup before Q1): panes padded 6 px top and bottom, split by a 1 px hairline.
    # [R1] Marc's R1 of 2026-11-14 replaces the mockup's sizes: title bar 46, 8 px between the tools panel and the
    # panes and between two panes, 14 px of padding on every side of a pane (the status line stays 26 [M]).
    # Content x = 236 + 8 = 244, 1240 - 244 = 996 wide, 760 - 46 - 26 = 688 tall; split: 688 - 8 = 680 = 340 + 340,
    # pane B from 46 + 340 + 8 = 394. Grid origin = rect + 14. Columns floor((996 - 28) / 8) = 121;
    # rows floor((340 - 28) / 20) = 15.
    exact(layout['inset'], [244, 46, 996, 688], 'inset')
    for key, rect, origin in [('a', [244, 46, 996, 340], [258, 60]),
                              ('b', [244, 394, 996, 340], [258, 408])]:
        pane = layout[key]
        exact(pane['rect'], rect, f'pane {key}')
        exact(pane['origin'], origin, f'grid {key} origin')
        exact([pane['cols'], pane['rows']], [121, 15], f'grid {key} size')
        if not scene_only:
            exact(layout['pty_' + key], [15, 121], f'PTY {key} size')
    # Tools panel hidden: the content is the full width; floor((1240 - 36) / 8) = 150 columns [Q1 L1].
    hidden = data['hidden']
    exact(hidden['inset'], [0, 38, 1240, 696], 'hidden inset')
    exact(hidden['a']['rect'], [0, 38, 1240, 347], 'hidden pane')
    exact([hidden['a']['cols'], hidden['a']['rows']], [150, 16], 'hidden grid size')
    # Divider dragged to y 300: the 1 px gap centred on the pointer, as the M12 design centred its 12 px gap;
    # pane A 300 - 0.5 - 38 = 261.5 -> 261 (the first pane takes the floor, as in the even split), B the rest
    # 695 - 261 = 434 from y 300. Rows floor((261 - 12) / 20) = 12 and floor((434 - 12) / 20) = 21.
    drag = data['drag']
    for key, rect, rows in [('a', [236, 38, 1004, 261], 12),
                            ('b', [236, 300, 1004, 434], 21)]:
        exact(drag[key]['rect'], rect, f'drag pane {key}')
        exact(drag[key]['rows'], rows, f'drag rows {key}')
        if not scene_only:
            exact(drag['restored_' + key]['rect'], rect, f'restored pane {key}')
    # Post-AI only took away the right column, which the redesign removed for everyone (Marc's R3).
    pane = data['post_ai']['a']
    exact(pane['rect'], [236, 38, 1004, 347], 'Post-AI pane')
    exact(pane['cols'], 121, 'Post-AI columns')
    coords = ['128,700', '600,4', '600,37', '235,200', '600,200', '600,385', '1100,200', '600,734', '600,750']
    # tools panel, title bar, its bottom hairline, the tools panel's right hairline, pane A, the split hairline,
    # where the right column was (now pane), the status line's top hairline, the status line [M].
    # Stone dark [T]: sidebar #0f0d0b, background #141210, border #fff0e1 at 20/255:
    # over the sidebar (34,31,28), over the background (38,35,32).
    side, bg, side_line, bg_line = [15,13,11,255], [20,18,16,255], [34,31,28,255], [38,35,32,255]
    dark = [side, side, side_line, side_line, bg, bg_line, bg, side_line, side]
    # Stone light [T]: sidebar #f3f0eb, background #fbfaf8, border #e7e1d9 (opaque).
    side, bg, line = [243,240,235,255], [251,250,248,255], [231,225,217,255]
    light = [side, side, line, line, bg, line, bg, line, side]
    pixels(data['pixels']['dark'], dict(zip(coords, dark)), 'Stone dark')
    pixels(data['pixels']['light'], dict(zip(coords, light)), 'Stone light')
    # The window gradient shows behind the title bar (y 20) and the status line (y 740), as it showed behind the
    # old sidebar.
    pixels(data['pixels']['bedazzled'], {'128,20':[75,30,149,255], '128,740':[31,57,138,255]}, 'AI Bedazzled')  # stops #4c1d95 -> #1e3a8a at t=0.023 / 0.977
    sizes = data['ui_text_px']
    if not isinstance(sizes, list) or not sizes or not all(type(x) in (int, float) and math.isfinite(x) and x >= 12 for x in sizes):
        raise Mismatch('UI text runs: expected nonempty list of finite sizes >= 12 px')
    if scene_only or no_idle:
        return
    idle = data['idle']
    exact(idle['no_input_frames'], 0, 'idle frames')
    frames = idle['key_echo_frames']
    if type(frames) is not int or not 1 <= frames <= 2:
        raise Mismatch(f'key echo frames: expected 1..2, observed {frames!r}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scene-only', action='store_true')
    parser.add_argument('--no-idle', action='store_true')
    args = parser.parse_args()
    try:
        compare(json.load(sys.stdin), args.scene_only, args.no_idle)
    except (Mismatch, KeyError, TypeError, ValueError) as error:
        print(f'FAIL: M12 partial comparisons: {error}', file=sys.stderr)
        return 1
    if args.scene_only:
        print('PASS: M12 scene comparisons only (PTYs, restore, idle and other checks not run)')
    elif args.no_idle:
        print('PASS: M12 layout, PTY, restore and scene comparisons only')
    else:
        print('PASS: M12 partial comparisons only (clicks, themes and Post-AI API not checked)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
