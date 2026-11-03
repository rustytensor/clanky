#!/usr/bin/env python3
"""M2 acceptance: fixed design inputs, real PTYs/parser/GPU, Python comparisons."""
import base64
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile
import zlib


CONFIG = {"font_size_pt": 12, "columns": 80, "rows": 24}
COLOURS = ((255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255))
NAMES = ("red", "green", "blue", "white")


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def png_chunk(kind, data):
    return (struct.pack('>I', len(data)) + kind + data
            + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff))


def image_input():
    # PNG scanlines are independently specified RGBA, filter type zero.
    scanlines = (b'\0\xff\0\0\xff\0\xff\0\xff'
                 b'\0\0\0\xff\xff\xff\xff\xff\xff')
    png = (b'\x89PNG\r\n\x1a\n'
           + png_chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 2, 8, 6, 0, 0, 0))
           + png_chunk(b'IDAT', zlib.compress(scanlines))
           + png_chunk(b'IEND', b''))
    return (b'\x1b[3;3H\x1b_Ga=T,f=100,t=d,i=1,c=2,r=2,q=2;'
            + base64.b64encode(png) + b'\x1b\\')


def private_write(path, content):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb') as file:
        file.write(content)


def observe(binary, option, path):
    process = subprocess.Popen([str(binary), option, str(path)],
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        stdout, stderr = process.communicate(timeout=30)
        require(process.returncode == 0,
                binary.name + ' failed: ' + stderr[-2000:])
        return json.loads(stdout)
    finally:
        # Saved PID only. SIGTERM lets the state helper shut down its Engine.
        if process.poll() is None:
            process.terminate()
            try:
                process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate(timeout=5)


def compare_images(observed, fault):
    if fault == 'image':
        # Alter only an observation, never the fixture or expected colours.
        observed['whole'][0] = [0, 0, 0, 255]
    for mode in ('whole', 'chunked'):
        samples = observed.get(mode)
        require(isinstance(samples, list) and len(samples) == 4,
                mode + ': expected four RGBA samples')
        for name, expected, sample in zip(NAMES, COLOURS, samples):
            label = mode + ' ' + name + ' quadrant'
            require(isinstance(sample, list) and len(sample) == 4,
                    label + ': expected RGBA sample')
            require(all(type(v) is int and 0 <= v <= 255 for v in sample),
                    label + ': invalid RGBA channel')
            require(sample[3] == 255, label + ': expected alpha 255 / observed ' + repr(sample[3]))
            require(all(abs(sample[i] - expected[i]) <= 8 for i in range(3)),
                    label + ': expected RGB %r +/-8 / observed %r' % (expected, sample[:3]))


def compare_state(state):
    require(state.get('config') == CONFIG, 'loaded config expected ' + repr(CONFIG))
    require(state.get('invalid_zero') is True, 'font size 0 must be a configuration error')
    expected_metrics = {
        'initial': {'font_px': 16, 'cell_width': 10, 'cell_height': 20},
        'zoom': {'font_px': 52 / 3, 'cell_width': 11, 'cell_height': 22},
        'reset': {'font_px': 16, 'cell_width': 10, 'cell_height': 20},
        'dpi': {'font_px': 24, 'cell_width': 15, 'cell_height': 30},
    }
    measured = state.get('metrics', {})
    for step, expected in expected_metrics.items():
        actual = measured.get(step, {})
        require(isinstance(actual.get('font_px'), (int, float))
                and abs(actual['font_px'] - expected['font_px']) < 0.0001,
                step + ': wrong physical font size: ' + repr(actual))
        for key in ('cell_width', 'cell_height'):
            require(actual.get(key) == expected[key],
                    step + ': expected %s %r / observed %r' % (key, expected[key], actual.get(key)))

    ids = state.get('ids')
    require(isinstance(ids, list) and len(ids) == 3
            and all(isinstance(i, str) and i for i in ids) and len(set(ids)) == 3,
            'expected three distinct real terminal IDs')
    left, top, bottom = ids

    def panes(key, pairs):
        expected = [{'id': i, 'rect': rect} for i, rect in pairs]
        require(state.get(key) == expected,
                key + ': expected %r / observed %r' % (expected, state.get(key)))

    panes('two_panes', [(left, [0, 0, 39, 24]), (top, [40, 0, 40, 24])])
    panes('rectangles', [(left, [0, 0, 39, 24]), (top, [40, 0, 40, 11]),
                         (bottom, [40, 12, 40, 12])])
    require(state.get('hit') == bottom, 'cell (41,13) must hit bottom-right terminal')
    require(state.get('divider') is None, 'cell (39,10) must hit no terminal')
    require(state.get('removed') is True, 'top-right layout leaf must close')
    panes('after_close', [(left, [0, 0, 39, 24]), (bottom, [40, 0, 40, 24])])
    require(sorted(state.get('surviving_ids', [])) == sorted([left, bottom]),
            'closing top-right must retain exactly the other terminal IDs')
    require(state.get('exit_codes') == [0, 0, 0], 'PTY exit codes must be 0/0/0')
    statuses = state.get('statuses')
    require(isinstance(statuses, list) and len(statuses) == 3, 'expected three PTY statuses')
    for status in statuses:
        require(status.get('done') is True and status.get('state') == 'exited'
                and status.get('signal') is None, 'PTY must exit normally and drain')
    outputs = state.get('outputs')
    require(isinstance(outputs, list) and len(outputs) == 3, 'expected three PTY outputs')
    for output, expected in zip(outputs, ('A', 'B', 'C')):
        require(output.get('encoding') == 'utf8' and output.get('data') == expected,
                'PTY raw output expected %r / observed %r' % (expected, output))
        require(output.get('eof') is True and output.get('truncated') is False
                and output.get('retained_start') == 0 and output.get('total_bytes') == 1
                and output.get('next_offset') == 1, 'PTY output incomplete/evicted')


def gate():
    require(len(sys.argv) == 1, 'usage: scripts/gate-m2.sh')
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'image'), 'unknown CLANKY_GATE_FAULT: ' + fault)
    target = Path(os.environ.get('CARGO_TARGET_DIR', str(Path(__file__).resolve().parents[1] / 'target'))).resolve()
    renderer = target / 'debug' / 'clanky-image-gate'
    state_binary = target / 'debug' / 'clanky-state-gate'
    for binary in (renderer, state_binary):
        require(binary.is_file() and os.access(binary, os.X_OK), 'missing executable: ' + str(binary))
    with tempfile.TemporaryDirectory(prefix='clanky-m2-', dir=target) as directory:
        root = Path(directory)
        os.chmod(root, 0o700)
        image = root / 'image.apc'
        config = root / 'config.json'
        private_write(image, image_input())
        private_write(config, json.dumps(CONFIG).encode('utf-8'))
        compare_images(observe(renderer, '--input', image), fault)
        print('1 APC/image pixels whole and two-byte PASS', flush=True)
        compare_state(observe(state_binary, '--config', config))
        print('2 split PTYs/config/DPI/zoom PASS', flush=True)
    env = os.environ.copy()
    env.pop('CLANKY_GATE_FAULT', None)
    m1 = Path(__file__).resolve().with_name('gate-m1.sh')
    result = subprocess.run([str(m1)], env=env, check=False)
    require(result.returncode == 0, 'M1 regression failed')
    print('3 M1 regression PASS', flush=True)


def interrupted(signum, frame):
    raise KeyboardInterrupt('signal %d' % signum)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, interrupted)
    try:
        gate()
    except (Exception, KeyboardInterrupt) as exc:
        print('FAIL: ' + str(exc), flush=True)
        sys.exit(1)
    print('PASS', flush=True)
