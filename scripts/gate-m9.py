#!/usr/bin/env python3
"""Exercise the real editor through a PTY; expectations are in design-m9.md."""
import fcntl
import json
import os
import pathlib
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

binary = pathlib.Path(sys.argv[1]).resolve()
build = pathlib.Path('/srv/devops/build/clanky-pkg')
build.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='m9-gate-', dir=build) as directory:
    work = pathlib.Path(directory)
    target, review, capture = (work / n for n in ('sample.txt', 'review.json', 'screen.txt'))
    target.write_bytes(b'alpha\nbravo\ncharlie\ndelta\n')
    review.write_text(json.dumps({'hunks': [
        {'start': 0, 'end': 5, 'replacement': 'ALPHA'},
        {'start': 12, 'end': 19, 'replacement': 'CHARLIE'},
    ]}))
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
    original = termios.tcgetattr(slave)
    env = dict(os.environ, TERM='xterm-256color', CLANKY_TUI_CAPTURE=str(capture))
    env.pop('CLANKY_GATE_FAULT', None)
    proc = subprocess.Popen([str(binary), 'edit', str(target) + ':2:3', '--review', str(review)],
                            stdin=slave, stdout=slave, stderr=slave, env=env,
                            start_new_session=True)
    output = bytearray()
    deadline = time.monotonic() + 30
    def pump():
        if time.monotonic() >= deadline:
            raise RuntimeError('PTY deadline exceeded')
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                output.extend(os.read(master, 65536))
            except OSError:
                pass
    try:
        while b'\x1b[?1049h' not in output:
            if proc.poll() is not None:
                raise RuntimeError(f'editor exited before startup: {proc.returncode}')
            pump()
        for keys in (b'X', b'\x1a', b'\x1b[15~', b'\x1b[17~', b'\x13'):
            os.write(master, keys)
            end = time.monotonic() + 0.15
            while time.monotonic() < end:
                pump()
        while not capture.exists():
            if proc.poll() is not None:
                raise RuntimeError(f'editor exited before save: {proc.returncode}')
            pump()
        os.write(master, b'\x11')
        while proc.poll() is None:
            pump()
        for _ in range(4):
            pump()
        if proc.returncode != 0:
            raise RuntimeError(f'editor exited {proc.returncode}')
        actual = target.read_bytes()
        if os.environ.get('CLANKY_GATE_FAULT') == 'tui':
            actual += b'fault'
        expected = b'ALPHA\nbravo\ncharlie\ndelta\n'
        if actual != expected:
            raise RuntimeError(f'saved bytes: expected {expected!r}, observed {actual!r}')
        rows = ['Clanky editor', 'ALPHA', 'bravo', 'charlie', 'delta'] + [''] * 18 + ['saved | Ctrl+S save | Ctrl+Q quit']
        screen = ''.join(row.ljust(80) + '\n' for row in rows).encode()
        if capture.read_bytes() != screen:
            raise RuntimeError('final 80x24 screen differs from fixed expectation')
        if termios.tcgetattr(slave) != original:
            raise RuntimeError('terminal settings not restored')
        if b'\x1b[?1049l' not in output or b'\x1b[?25h' not in output:
            raise RuntimeError('alternate screen or cursor not restored')
    except (RuntimeError, OSError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        sys.exit(1)
    finally:
        if proc.poll() is None:
            proc.send_signal(signal.SIGTERM)
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        os.close(master)
        os.close(slave)
