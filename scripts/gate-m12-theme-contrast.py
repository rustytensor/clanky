#!/usr/bin/env python3
"""WCAG 2.x semantic text contrast on actual M12 component surfaces, at opacity 1.

Input: clanky-theme-gate --palettes (the real built-in loader, not a directory glob).
No tolerance below 4.5. Disabled controls and program-selected ANSI colours are not
semantic text tokens. Exemptions are explicit IDs, never name/prefix matching.
"""
import json
import math
import os
import sys

TOKENS = ('foreground', 'muted_foreground', 'primary_foreground', 'ok', 'err', 'agent', 'background (cursor)')
EXEMPT = {
    'ai-bedazzled': 'design joke-theme exemption',
    'unicorn-vomit': 'design joke-theme exemption',
    'catppuccin-latte': 'lead: preserve exact upstream colours',
    'catppuccin-frappe': 'lead: preserve exact upstream colours',
    'catppuccin-macchiato': 'lead: preserve exact upstream colours',
    'catppuccin-mocha': 'lead: preserve exact upstream colours',
}
SURFACES = ('sidebar', 'background', 'card', 'muted', 'accent', 'pressed', 'sidebar_accent')


def rgba(text):
    channels = [int(text[i:i+2], 16) / 255 for i in range(1, len(text), 2)]
    return channels if len(channels) == 4 else channels + [1.0]


def over(color, surface):
    return [color[i] * color[3] + surface[i] * (1 - color[3]) for i in range(3)]


def luminance(rgb):
    def linear(v):
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    return sum(w * linear(v) for w, v in zip((0.2126, 0.7152, 0.0722), rgb))


def contrast(text, surface):
    a, b = sorted((luminance(over(text, surface)), luminance(surface)))
    return (b + 0.05) / (a + 0.05)


def gradient_ratio(text, a, b):
    # With opaque text, luminance along an sRGB segment is convex. Its maximum
    # is at an endpoint; find its minimum, then the closest background luminance
    # to the text. This checks the continuous gradient, not a few sample rows.
    if text[3] != 1:
        raise ValueError('gradient text must be opaque for the continuous contrast check')
    def lum(t):
        return luminance([x * (1-t) + y*t for x, y in zip(a, b)])
    lo, hi = 0.0, 1.0
    for _ in range(64):
        left, right = (2*lo+hi)/3, (lo+2*hi)/3
        if lum(left) <= lum(right):
            hi = right
        else:
            lo = left
    minimum = min(lum(0), lum(1), lum((lo+hi)/2))
    maximum = max(lum(0), lum(1))
    ink = luminance(text[:3])
    nearest = min(max(ink, minimum), maximum)
    return (max(ink, nearest)+0.05)/(min(ink, nearest)+0.05)


def pairs(theme):
    c = {key: rgba(value) for key, value in theme['colors'].items()}
    if c['background'][3] != 1:
        raise ValueError('background must be opaque at the fixed gate opacity')
    surfaces = {key: over(c[key], c['background'][:3]) for key in SURFACES}
    # Main/secondary labels cover normal, hover, pressed, sidebar and card fills.
    for token in ('foreground', 'muted_foreground'):
        for surface in SURFACES:
            yield f'{token}/{surface}', contrast(c[token], surfaces[surface])
    # Status/command text and 10% status badges on panes, cards and command headers.
    for token in ('ok', 'err', 'agent'):
        for surface in ('background', 'card', 'muted'):
            base = surfaces[surface]
            yield f'{token}/{surface}', contrast(c[token], base)
            tint = c[token][:3] + [math.floor(c[token][3]*255*0.1+0.5)/255]
            yield f'{token}/{surface}+10% {token}', contrast(c[token], over(tint, base))
    primary = over(c['primary'], surfaces['background'])
    pressed = over(c['primary_pressed'], surfaces['background'])
    hover = [math.floor((c['primary'][i]*0.9+c['background'][i]*0.1)*255+0.5)/255 for i in range(4)]
    for name, surface in [('primary', primary), ('primary_pressed', pressed),
                          ('primary_hover', over(hover, surfaces['background']))]:
        yield f'primary_foreground/{name}', contrast(c['primary_foreground'], surface)
    # Text in a reverse cursor uses the background token on the foreground fill.
    yield 'background/foreground (cursor)', contrast(c['background'], over(c['foreground'], surfaces['background']))
    stops = theme['window_gradient']
    if stops:
        stops = [over(rgba(stop), surfaces['background']) for stop in stops]
        for token in ('foreground', 'muted_foreground'):
            for i, (a, b) in enumerate(zip(stops, stops[1:])):
                yield f'{token}/window_gradient[{i}]', gradient_ratio(c[token], a, b)


def main():
    documents = json.load(sys.stdin)
    if not isinstance(documents, list) or not documents:
        raise ValueError('expected a nonempty built-in theme list')
    seen = set()
    failures = 0
    checked = 0
    for entry in documents:
        identity, theme = entry['id'], entry['theme']
        if identity in seen:
            raise ValueError(f'duplicate theme ID: {identity}')
        seen.add(identity)
        if identity in EXEMPT:
            print(f'EXEMPT: {identity}: {", ".join(TOKENS)}; {EXEMPT[identity]}')
            continue
        if os.environ.get('CLANKY_THEME_GATE_FAULT') == 'contrast' and identity == 'stone-dark':
            theme['colors']['foreground'] = theme['colors']['background']
        results = list(pairs(theme))
        checked += 1
        bad = [(label, ratio) for label, ratio in results if not math.isfinite(ratio) or ratio < 4.5]
        label, minimum = min(results, key=lambda pair: pair[1])
        print(f'{"FAIL" if bad else "PASS"}: {identity}: minimum {minimum:.4f}:1 at {label}; {len(bad)} failing pairs')
        for label, ratio in bad:
            print(f'  FAIL: {identity} {label}: {ratio:.4f}:1 < 4.5:1')
        failures += len(bad)
    if not checked:
        raise ValueError('no nonexempt themes were checked')
    print(f'{"FAIL" if failures else "PASS"}: theme contrast: {checked} own themes; {failures} failing pairs; threshold 4.5:1')
    return int(failures != 0)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, KeyError, TypeError, IndexError) as error:
        print(f'FAIL: theme contrast input: {error}', file=sys.stderr)
        sys.exit(1)
