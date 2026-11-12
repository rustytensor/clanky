#!/usr/bin/env python3
"""T1 gate: darker dark themes, a new Ink theme, readable joke themes (Marc approved 2026-11-12).

Input: clanky-theme-gate --palettes (the real built-in loader). Expected values, fixed before the work:
- Built-in IDs, in this order: EXPECTED_IDS (14 themes, Ink after Graphite).
- Pane backgrounds: APPROVED_BACKGROUNDS, the values on the approved mockup.
- Gradients: APPROVED_GRADIENTS for the two joke themes.
- Every theme except Catppuccin (upstream colours, kept exactly): every interface text pair of the M12
  contrast gate at 4.5:1 or more, with no joke-theme exemption; ANSI colours 1-7 and 9-15 at 4.5:1 or more
  on the pane background, ANSI 8 (bright black, used for prompts and hints) at 3:1 or more.
CLANKY_T1_FAULT=gradient|ansi|background plants one fault in the input; the gate must fail on each.
"""
import importlib.util
import json
import os
import pathlib
import sys

spec = importlib.util.spec_from_file_location(
    'm12', pathlib.Path(__file__).with_name('gate-m12-theme-contrast.py'))
m12 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m12)

EXPECTED_IDS = [
    'stone-dark', 'stone-light', 'paper-dark', 'paper-light', 'graphite-dark', 'graphite-light', 'ink',
    'catppuccin-latte', 'catppuccin-frappe', 'catppuccin-macchiato', 'catppuccin-mocha',
    'ai-bedazzled', 'unicorn-tears', 'unicorn-vomit',
]
APPROVED_BACKGROUNDS = {
    'ink': '#121212', 'stone-dark': '#141210', 'paper-dark': '#1f1c17', 'graphite-dark': '#111111',
}
APPROVED_GRADIENTS = {
    'unicorn-vomit': ['#7a0019', '#7a3000', '#524000', '#005522', '#002a7a', '#4a0088'],
    'ai-bedazzled': ['#4c1d95', '#1e3a8a'],
}
UPSTREAM = {'catppuccin-latte', 'catppuccin-frappe', 'catppuccin-macchiato', 'catppuccin-mocha'}


def norm(color):
    color = color.lower()
    return color[:7] if len(color) == 9 and color.endswith('ff') else color


def main():
    documents = json.load(sys.stdin)
    themes = {entry['id']: entry['theme'] for entry in documents}
    fault = os.environ.get('CLANKY_T1_FAULT', '')
    if fault == 'gradient':
        themes['unicorn-vomit']['window_gradient'] = ['#ff0000', '#ff8000', '#ffff00', '#00ff00', '#0000ff', '#8000ff']
    elif fault == 'ansi' and 'ink' in themes:
        themes['ink']['ansi'][1] = '#3a1010'
    elif fault == 'background':
        themes['stone-dark']['colors']['background'] = '#1a1816'
    elif fault:
        raise ValueError(f'unknown fault {fault}')

    failures = []
    ids = [entry['id'] for entry in documents]
    if ids != EXPECTED_IDS:
        failures.append(f'built-in IDs {ids}')
    for identity, expected in APPROVED_BACKGROUNDS.items():
        got = norm(themes.get(identity, {}).get('colors', {}).get('background', 'missing'))
        if got != expected:
            failures.append(f'{identity} background {got}, approved {expected}')
    for identity, expected in APPROVED_GRADIENTS.items():
        got = [norm(c) for c in (themes.get(identity, {}).get('window_gradient') or [])]
        if got != expected:
            failures.append(f'{identity} gradient {got}, approved {expected}')
    for identity, theme in themes.items():
        if identity in UPSTREAM:
            continue
        ui = [(label, ratio) for label, ratio in m12.pairs(theme) if ratio < 4.5]
        failures += [f'{identity} {label} {ratio:.2f}:1 < 4.5:1' for label, ratio in ui]
        background = m12.rgba(theme['colors']['background'])[:3]
        for index, color in enumerate(theme['ansi']):
            if index == 0:
                continue
            need = 3.0 if index == 8 else 4.5
            ratio = m12.contrast(m12.rgba(color), background)
            if ratio < need:
                failures.append(f'{identity} ansi[{index}] {color} {ratio:.2f}:1 < {need}:1')
        print(f'checked: {identity}')
    for failure in failures:
        print(f'FAIL: {failure}')
    if failures:
        print(f'FAIL: T1 themes ({len(failures)} problems)')
        return 1
    print(f'PASS: T1 themes ({len(themes) - len(UPSTREAM)} own themes, Catppuccin kept upstream)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
