#!/usr/bin/env python3
"""Write and read Clanky's config.xml for the gate harnesses (format of docs/config.md, Clanky 1.7).

One root <clanky version="1">, one element per setting, text values:
  booleans true | false; numbers in decimal; strings as text (XML-escaped);
  tools            <tools><tool>files</tool><tool>agents</tool></tools>
  colors           <colors><accent>#3a7bd5</accent></colors>           (one element per colour token)
  terminal_colors  <terminal_colors><background>#..</background><foreground>#..</foreground><cursor>#..</cursor>
                   <ansi><color>#..</color> x 16, in order 0..15</ansi></terminal_colors>
  agents           <agents><agent><name>Claude Code</name><command>claude</command></agent>...</agents>
The legacy keys font_size_pt and rails are written as plain elements, as older gates pass them.

dump(dict) -> text, write(path, dict), loads(text) -> dict, load(path) -> dict (empty dict when the file is missing
or not well-formed, as the harnesses' saved() did for config.json). loads() gives the same Python values json.loads
gave for the old config.json: booleans as bool, integers as int, other numbers as float.
"""
import xml.etree.ElementTree as ET
from pathlib import Path
from xml.sax.saxutils import escape

BOOL = {'post_ai', 'startup_animation', 'compose_send_enter', 'osc52_write', 'osc52_read', 'restore_layout',
        'animations', 'rails', 'history'}
INT = {'columns', 'rows', 'notify_after_seconds', 'history_days', 'history_output_kib', 'cursor_blink_time_ms',
       'cursor_blink_timeout_s'}
NUM = {'font_size', 'font_size_pt', 'opacity', 'cell_width_scale', 'cell_height_scale'}
TERMINAL_COLOR_KEYS = ('background', 'foreground', 'cursor')


def _scalar(value):
    if isinstance(value, bool):
        return 'true' if value else 'false'
    if isinstance(value, float):
        return repr(value)
    return escape(str(value))


def dump(cfg):
    out = ['<?xml version="1.0" encoding="UTF-8"?>', '<clanky version="1">']
    for key, value in cfg.items():
        if key == 'tools':
            out.append('  <tools>' + ''.join(f'<tool>{escape(t)}</tool>' for t in value) + '</tools>')
        elif key == 'colors':
            out.append('  <colors>' + ''.join(f'<{k}>{escape(v)}</{k}>' for k, v in value.items()) + '</colors>')
        elif key == 'terminal_colors':
            inner = ''.join(f'<{k}>{escape(value[k])}</{k}>' for k in TERMINAL_COLOR_KEYS if k in value)
            if 'ansi' in value:
                inner += '<ansi>' + ''.join(f'<color>{escape(c)}</color>' for c in value['ansi']) + '</ansi>'
            out.append(f'  <terminal_colors>{inner}</terminal_colors>')
        elif key == 'agents':
            out.append('  <agents>' + ''.join(f'<agent><name>{escape(a["name"])}</name><command>{escape(a["command"])}'
                                              f'</command></agent>' for a in value) + '</agents>')
        elif isinstance(value, (list, dict)):
            raise ValueError(f'no XML form for {key}')
        else:
            out.append(f'  <{key}>{_scalar(value)}</{key}>')
    out.append('</clanky>')
    return '\n'.join(out) + '\n'


def write(path, cfg):
    Path(path).write_text(dump(cfg))


def _number(text):
    text = text.strip()
    try:
        return int(text)
    except ValueError:
        return float(text)


def element_value(el):
    """The Python value of one setting element, as json.loads gave it from config.json."""
    key, text = el.tag, (el.text or '').strip()
    if key == 'tools':
        return [(t.text or '').strip() for t in el]
    if key == 'colors':
        return {c.tag: (c.text or '').strip() for c in el}
    if key == 'terminal_colors':
        out = {}
        for c in el:
            if c.tag == 'ansi':
                out['ansi'] = [(a.text or '').strip() for a in c]
            else:
                out[c.tag] = (c.text or '').strip()
        return out
    if key == 'agents':
        return [{f.tag: (f.text or '').strip() for f in a} for a in el]
    if key in BOOL:
        return {'true': True, 'false': False}.get(text, text)
    if key in INT or key in NUM:
        try:
            return _number(text)
        except ValueError:
            return text
    return text


def loads(text):
    root = ET.fromstring(text)
    if root.tag != 'clanky':
        raise ValueError(f'root element is <{root.tag}>, not <clanky>')
    return {el.tag: element_value(el) for el in root}


def load(path):
    try:
        return loads(Path(path).read_text())
    except (OSError, ValueError, ET.ParseError):
        return {}
