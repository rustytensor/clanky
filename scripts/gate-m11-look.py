#!/usr/bin/env python3
"""Compare M11 observations with the independently specified Studio gate.

The harness supplies one JSON object on stdin. Coordinates and dimensions are
physical pixels at 1240x760, scale 1; grids use 10x20 cells. The default
terminal and UI fonts are bundled JetBrains Mono and Inter respectively;
`font_family` and `ui_font_family` can name other installed families. Its
schema is:

  layout: {content: [x,y,w,h], a: {rect, origin, cols, rows}, b: {rect, origin, cols, rows},
           pty_a: [rows, cols], pty_b: [rows, cols]}
  rails_hidden: {a: {rect, cols, rows}, b: {rect}}
  drag: {a: {rect, rows}, b: {rect, rows},
         restored_a: {rect}, restored_b: {rect}}
  pixels: {"x,y": [r,g,b,a], ...}  (first scene, premultiplied RGBA)
  agent_glow: [r,g,b,a]             (second scene at 215,222)
  motion: {cursor: [0ms,40ms,80ms,120ms], panel_60ms,
           cursor_disabled_0ms, panel_disabled_0ms}
  config: {opacity_0_3_error: bool, rails_yes_error: bool,
           opacity_one_pixel: [r,g,b,a]}
  light_pixels: {"x,y": [r,g,b,a], ...}
  light_agent_glow: [r,g,b,a]
  settings: {opacity_readback: number, opacity_pixel: [r,g,b,a],
             invalid_error: string, loader_error: string, invalid_unchanged: bool,
             panel_override_pixel: [r,g,b,a], export_import_identical: bool,
             key_order_preserved: bool}

`--non-pixel` checks layout, drag, motion and the two config errors before the
renderer exists. It deliberately does not claim the complete gate passed.
"""
import argparse
import json
import math
import sys


class Mismatch(Exception):
    pass


def require(condition, label, expected, observed):
    if not condition:
        raise Mismatch(f"{label}: expected {expected!r}, observed {observed!r}")


def exact(actual, expected, label):
    # JSON booleans compare equal to integers in Python; they are not numbers here.
    if isinstance(expected, bool):
        require(type(actual) is bool and actual is expected, label, expected, actual)
    elif isinstance(expected, (int, float)):
        require(type(actual) in (int, float) and math.isfinite(actual) and actual == expected,
                label, expected, actual)
    elif isinstance(expected, list):
        require(isinstance(actual, list) and len(actual) == len(expected), label, expected, actual)
        for index, value in enumerate(expected):
            exact(actual[index], value, f"{label}[{index}]")
    else:
        require(actual == expected, label, expected, actual)


def rgba(actual, label):
    require(isinstance(actual, list) and len(actual) == 4 and
            all(type(value) is int and 0 <= value <= 255 for value in actual),
            label, "four integer channels in 0..255", actual)
    return actual


def pixel(actual, expected, label, tolerance=3):
    actual = rgba(actual, label)
    require(all(abs(a - b) <= tolerance for a, b in zip(actual, expected)),
            label, f"{expected} ±{tolerance} per channel", actual)


def compare(data, non_pixel=False, dark_only=False):
    exact(data['layout']['content'], [220, 50, 740, 700], 'content area')
    a, b = data['layout']['a'], data['layout']['b']
    exact(a['rect'], [220, 50, 740, 345], 'pane A')
    exact(b['rect'], [220, 405, 740, 345], 'pane B')
    exact(a['origin'], [232, 86], 'grid A origin')
    exact(b['origin'], [232, 441], 'grid B origin')
    for name, pane in [('A', a), ('B', b)]:
        exact(pane['cols'], 71, f'grid {name} columns')
        exact(pane['rows'], 14, f'grid {name} rows')
    exact(data['layout']['pty_a'], [14, 71], 'PTY A rows, columns')
    exact(data['layout']['pty_b'], [14, 71], 'PTY B rows, columns')

    hidden = data['rails_hidden']
    exact(hidden['a']['rect'], [10, 50, 1220, 345], 'hidden rails pane A')
    exact(hidden['a']['cols'], 119, 'hidden rails pane A columns')
    exact(hidden['a']['rows'], 14, 'hidden rails pane A rows')
    exact(hidden['b']['rect'], [10, 405, 1220, 345], 'hidden rails pane B')

    drag = data['drag']
    exact(drag['a']['rect'], [220, 50, 740, 245], 'drag pane A')
    exact(drag['a']['rows'], 9, 'drag pane A rows')
    exact(drag['b']['rect'], [220, 305, 740, 445], 'drag pane B')
    exact(drag['b']['rows'], 19, 'drag pane B rows')
    exact(drag['restored_a']['rect'], [220, 50, 740, 245], 'restored pane A')
    exact(drag['restored_b']['rect'], [220, 305, 740, 445], 'restored pane B')

    if not non_pixel:
        pixels = data['pixels']
        for coord, expected, name in [
            ('590,44', [14, 20, 32, 240], 'gap above pane A'),
            ('950,380', [19, 27, 43, 255], 'pane A padding'),
            ('221,51', [14, 20, 32, 240], 'pane A rounded corner'),
            ('226,56', [19, 27, 43, 255], 'pane A inside corner'),
            ('100,700', [11, 17, 28, 255], 'left rail'),
            ('1100,700', [11, 17, 28, 255], 'right rail'),
            ('230,451', [26, 34, 49, 255], 'block card'),
            ('936,451', [38, 66, 66, 255], 'exit chip'),
        ]:
            pixel(pixels[coord], expected, f'{name} ({coord})')
        glow = rgba(pixels['215,577'], 'focus glow (215,577)')
        require(glow[2] >= 40 and glow[2] > glow[0], 'focus glow (215,577)',
                'blue >= 40 and blue > red', glow)
        glow = rgba(data['agent_glow'], 'agent glow (215,222)')
        require(glow[0] >= glow[1] + 20 and glow[2] >= glow[1] + 20,
                'agent glow (215,222)', 'red and blue >= green + 20', glow)

    motion = data['motion']
    exact(motion['cursor'], [100, 187.5, 200, 200], 'cursor easing')
    exact(motion['panel_60ms'], 0.875, 'panel easing at 60 ms')
    exact(motion['cursor_disabled_0ms'], 200, 'cursor with animations disabled')
    exact(motion['panel_disabled_0ms'], 1.0, 'panel with animations disabled')
    config = data['config']
    exact(config['opacity_0_3_error'], True, 'opacity 0.3 rejected')
    exact(config['rails_yes_error'], True, 'rails string rejected')
    if not non_pixel:
        observed = rgba(config['opacity_one_pixel'], 'opacity 1 pixel (590,44)')
        exact(observed[3], 255, 'opacity 1 alpha (590,44)')

        if dark_only:
            return
        light = data['light_pixels']
        for coord, expected, name in [
            ('590,44', [219, 224, 232, 240], 'gap above pane A'),
            ('950,380', [255, 255, 255, 255], 'pane A padding'),
            ('221,51', [219, 224, 232, 240], 'pane A rounded corner'),
            ('226,56', [255, 255, 255, 255], 'pane A inside corner'),
            ('100,700', [243, 246, 251, 255], 'left rail'),
            ('1100,700', [243, 246, 251, 255], 'right rail'),
            ('230,451', [246, 246, 247, 255], 'block card'),
            ('936,451', [224, 234, 229, 255], 'exit chip'),
        ]:
            pixel(light[coord], expected, f'light {name} ({coord})')
        glow = rgba(light['215,577'], 'light focus glow (215,577)')
        require(glow[2] - glow[0] >= 21, 'light focus glow (215,577)',
                'blue >= red + 21', glow)
        glow = rgba(data['light_agent_glow'], 'light agent glow (215,222)')
        require(glow[1] <= 214 and glow[2] >= glow[1] + 18,
                'light agent glow (215,222)', 'green <= 214 and blue >= green + 18', glow)

        settings = data['settings']
        exact(settings['opacity_readback'], 0.8, 'settings opacity readback')
        observed = rgba(settings['opacity_pixel'], 'settings opacity pixel (590,44)')
        exact(observed[3], 204, 'settings opacity alpha (590,44)')
        error = settings['invalid_error']
        require(isinstance(error, str) and bool(error), 'settings invalid opacity error',
                'nonempty loader error', error)
        exact(error, settings['loader_error'], 'settings error matches loader')
        exact(settings['invalid_unchanged'], True, 'invalid settings leave file unchanged')
        pixel(settings['panel_override_pixel'], [32, 32, 32, 255],
              'settings panel override (950,380)')
        exact(settings['export_import_identical'], True, 'settings export and import')
        exact(settings['key_order_preserved'], True, 'settings key order')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--non-pixel', action='store_true')
    parser.add_argument('--dark-only', action='store_true')
    args = parser.parse_args()
    try:
        data = json.load(sys.stdin)
        compare(data, args.non_pixel, args.dark_only)
    except (Mismatch, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f'FAIL: M11 look: {error}', file=sys.stderr)
        return 1
    if args.non_pixel:
        print('PASS: M11 non-pixel comparisons (pixel checks not run)')
    elif args.dark_only:
        print('PASS: M11 dark comparisons (light and settings not run)')
    else:
        print('PASS: M11 look comparisons')
    return 0


if __name__ == '__main__':
    sys.exit(main())
