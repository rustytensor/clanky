#!/usr/bin/env python3
"""Convert all M12 premultiplied theme renderings to straight-alpha PNGs."""
import pathlib
import struct
import sys
import zlib

work, target = map(pathlib.Path, sys.argv[1:])


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


images = sorted(work.glob('*.rgba'))
if len(images) != 14:
    raise SystemExit(f'FAIL: expected 14 theme pictures, found {len(images)}')
w, h = 1240, 760
for image in images:
    pixels = bytearray(image.read_bytes())
    if len(pixels) != w * h * 4:
        raise SystemExit('FAIL: invalid picture size')
    for i in range(0, len(pixels), 4):
        alpha = pixels[i + 3]
        if alpha:
            for c in range(3):
                pixels[i+c] = min(255, (pixels[i+c] * 255 + alpha // 2) // alpha)
    rows = b''.join(b'\0' + pixels[y*w*4:(y+1)*w*4] for y in range(h))
    png = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB',w,h,8,6,0,0,0))
           + chunk(b'IDAT',zlib.compress(rows)) + chunk(b'IEND',b''))
    (target / (image.stem + '.png')).write_bytes(png)
print('PASS: wrote 14 theme pictures')
