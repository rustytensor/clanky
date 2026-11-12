#!/usr/bin/env python3
"""Settings observations checked against m12-settings-expectations.md."""
import json
import math
import re
import sys


def require(ok, message):
    if not ok:
        raise ValueError(message)


def main():
    data=json.load(sys.stdin)
    for name,expected in [('panel',[230,80,780,600]),('nav',[231,81,200,598]),
                          ('header',[431,81,578,56]),('footer',[431,623,578,56]),
                          ('panel_radii',[12,12])]:
        require(data[name]==expected,f'{name}: expected {expected}, observed {data[name]}')
    for theme,values in [('stone-dark',{'nav':[15,13,11,255],'header':[20,18,16,255]}),
                         ('stone-light',{'nav':[243,240,235,255],'header':[251,250,248,255]})]:
        for point,expected in values.items():
            value=data['pixels'][theme][point]
            require(len(value)==4 and all(type(x) is int for x in value) and
                    all(abs(x-y)<=3 for x,y in zip(value,expected)),
                    f'{theme} {point}: expected {expected} ±3, observed {value}')
    rows=data['row_heights']
    require(len(rows)>=6 and all(type(x) in (int,float) and math.isfinite(x) and x>=64 for x in rows),
            f'setting rows: expected >=64 px, observed {rows}')
    gaps=data['row_gaps']
    require(len(gaps)>=8 and all(type(x) in (int,float) and math.isfinite(x) and x>=64 for x in gaps),
            f'setting label spacing: expected >=64 px, observed {gaps}')
    labels=data['labels']
    require(isinstance(labels,list) and labels and all(isinstance(x,str) for x in labels),'Settings labels missing')
    invalid=[text for text in labels if re.search(r'(?<![\w.])(?:true|false|1\.00)(?![\w.])',text,re.I)]
    require(not invalid,f'raw values in Settings labels: {invalid}')
    print(f'PASS: Settings 780×600 radius 12, 200 px navigation, 56 px title/footer; {len(rows)} row targets and {len(gaps)} label gaps ≥64 px; {len(labels)} labels')


if __name__=='__main__':
    try:main()
    except (ValueError,KeyError,TypeError) as error:
        print(f'FAIL: M12 Settings geometry: {error}',file=sys.stderr);sys.exit(1)
