#!/usr/bin/env python3
"""Measures a Clanky binary's speed from outside, the same way for any build, on a private Xvfb display.

  speed.py BINARY [--runs 3] [--scale 1] [--keys 200] [--out FILE.json]

Five numbers per run, then the median and the spread (lowest..highest) over the runs:
  start      launch to the first drawing on screen (ms)
  memory     resident memory of every process running BINARY, 2 s after the first frame (MiB)
  idle       drawing bursts seen in 10 s with nothing happening (cursor blink and animations off)
  key p50/99 XTest key press to the echoed character changing on screen (ms), over --keys keys
  cat        time for `cat` of 100 MB of mixed text to finish writing into the pane (s)

With --scene palette the window first gets two tabs and four panes (Ctrl+Shift+D, T, D) and the command palette
(Ctrl+Shift+P), and a sixth number is added: the time from that key press to the palette's first frame (ms). Then
memory and idle are measured with the palette open, the key number is the time from a typed letter or a Backspace
to the palette's changed pixels, and cat runs in a pane behind it.

The pane runs a stand-in shell (SHELL points at a small program in the work folder) that echoes each key back at
the start of the line and, when told, runs cat on the text file and records how long it took. Drawing is seen
through the X DAMAGE extension on the root window; a key counts as shown when the pixels where the echo lands
have changed. The 100 MB file is made once from a fixed seed, so every run and every binary reads the same bytes.
"""

import argparse
import hashlib
import json
import os
import random
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import clanky_config_xml as config_xml  # noqa: E402
import xprobe  # noqa: E402

DATA_DIR = Path('/srv/devops/build/clanky-g1-speed')
TEXT_FILE = DATA_DIR / 'mixed-100mb.txt'
TEXT_BYTES = 100_000_000
TEXT_SEED = 20261117
IDLE_SECONDS = 10
SCENE_CHORDS = (
    ('Control_L', 'Shift_L', 'd'),  # split right: two panes in the first tab
    ('Control_L', 'Shift_L', 't'),  # a second tab
    ('Control_L', 'Shift_L', 'd'),  # two panes in it: four in all
)
PALETTE_CHORD = ('Control_L', 'Shift_L', 'p')
SETTLE_SECONDS = 15
SCREEN = (1600, 1000)

STAND_IN_SHELL = r"""#!/usr/bin/python3 -I
import os, select, subprocess, sys, time, tty
from pathlib import Path
here = Path(__file__).resolve().parent
tty.setraw(0)
os.write(1, b'\x1b[2J\x1b[Hready\r\n')
(here / 'shell-ready').write_text('ready')
while True:
    readable, _, _ = select.select([0], [], [], 0.05)
    if readable:
        data = os.read(0, 64)
        if not data:
            break
        for byte in data:
            os.write(1, b'\r' + bytes([byte]))
    order = here / 'cat-go'
    if order.exists():
        order.unlink()
        started = time.monotonic()
        subprocess.run(['cat', str(here / 'cat-file')], check=True)
        took = time.monotonic() - started
        (here / 'cat-took').write_text(f'{took:.6f}\n')
"""


def make_text_file():
    """Writes the mixed text once: SGR colours (16, 256 and true colour), short and long lines, some CJK."""
    if TEXT_FILE.exists() and TEXT_FILE.stat().st_size == TEXT_BYTES:
        return
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    rng = random.Random(TEXT_SEED)
    words = [''.join(rng.choice('abcdefghijklmnopqrstuvwxyz') for _ in range(rng.randint(1, 10))) for _ in range(500)]
    wide = [chr(c) for c in range(0x4E00, 0x4E00 + 400)]
    chunks, size = [], 0
    while size < TEXT_BYTES:
        kind = rng.random()
        if kind < 0.45:  # short line
            line = ' '.join(rng.choice(words) for _ in range(rng.randint(1, 4)))
        elif kind < 0.75:  # long line that wraps
            line = ' '.join(rng.choice(words) for _ in range(rng.randint(40, 300)))
        elif kind < 0.85:  # wide characters
            line = ''.join(rng.choice(wide) for _ in range(rng.randint(5, 60)))
        else:  # coloured words
            parts = []
            for _ in range(rng.randint(3, 20)):
                style = rng.choice(
                    [
                        f'\x1b[{rng.randint(30, 37)}m',
                        f'\x1b[1;{rng.randint(90, 97)}m',
                        f'\x1b[38;5;{rng.randint(0, 255)}m',
                        f'\x1b[38;2;{rng.randint(0, 255)};{rng.randint(0, 255)};{rng.randint(0, 255)}m',
                        f'\x1b[48;5;{rng.randint(0, 255)}m',
                    ]
                )
                parts.append(style + rng.choice(words) + '\x1b[0m')
            line = ' '.join(parts)
        data = (line + '\n').encode()
        chunks.append(data)
        size += len(data)
    blob = b''.join(chunks)[:TEXT_BYTES]
    partial = TEXT_FILE.with_suffix('.partial')
    partial.write_bytes(blob)
    partial.rename(TEXT_FILE)


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def until(check, label, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = check()
        if value:
            return value
        time.sleep(0.01)
    raise RuntimeError(f'timed out after {seconds} s waiting for {label}')


def resident_mib(binary):
    """Adds up VmRSS of every process whose executable is `binary`."""
    target, total = os.path.realpath(binary), 0
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            if os.path.realpath(entry / 'exe') != target:
                continue
            for line in (entry / 'status').read_text().splitlines():
                if line.startswith('VmRSS:'):
                    total += int(line.split()[1])
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue  # the process ended while being read
    return total / 1024


def processes_of(binary):
    target, found = os.path.realpath(binary), []
    for entry in Path('/proc').iterdir():
        if entry.name.isdigit():
            try:
                if os.path.realpath(entry / 'exe') == target:
                    found.append(int(entry.name))
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                continue
    return found


def stop_pids(pids):
    for pid in pids:
        try:
            os.kill(pid, 15)
        except ProcessLookupError:
            continue
    end = time.monotonic() + 5
    while time.monotonic() < end and any(Path(f'/proc/{pid}').exists() for pid in pids):
        time.sleep(0.05)
    for pid in pids:
        if Path(f'/proc/{pid}').exists():
            try:
                os.kill(pid, 9)
            except ProcessLookupError:
                continue


def changed_box(before, after):
    rows, cols = np.nonzero(np.any(before != after, axis=2))
    if rows.size == 0:
        return None
    return int(cols.min()), int(rows.min()), int(cols.max()) + 1, int(rows.max()) + 1


class Session:
    """One Xvfb display, one window of the binary under test, with a private home."""

    def __init__(self, binary, scale, work):
        self.binary, self.work = binary, work
        for name in ('home', 'config', 'state', 'runtime'):
            (work / name).mkdir(mode=0o700)
        shell = work / 'stand-in-shell'
        shell.write_text(STAND_IN_SHELL)
        shell.chmod(0o700)
        (work / 'cat-file').symlink_to(TEXT_FILE)
        config = work / 'config.xml'
        config_xml.write(
            config,
            {
                'columns': 120,
                'rows': 36,
                'animations': False,
                'startup_animation': False,
                'restore_layout': False,
                'osc52_read': False,
                'osc52_write': False,
                'cursor_blink': 'off',
                'tools_panel': 'off',
            },
        )
        self.display = xprobe.free_display()
        self.xvfb = subprocess.Popen(
            ['Xvfb', self.display, '-screen', '0', f'{SCREEN[0]}x{SCREEN[1]}x24', '-nolisten', 'tcp'],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        number = self.display[1:]
        socket = Path(f'/tmp/.X11-unix/X{number}')  # noqa: S108 - the X server's own socket path
        until(socket.exists, 'Xvfb', 10)
        self.x = xprobe.Display(self.display)
        self.env = {
            'PATH': '/usr/bin:/bin',
            'HOME': str(work / 'home'),
            'XDG_CONFIG_HOME': str(work / 'config'),
            'XDG_STATE_HOME': str(work / 'state'),
            'XDG_RUNTIME_DIR': str(work / 'runtime'),
            'DISPLAY': self.display,
            'SHELL': str(shell),
            'WINIT_UNIX_BACKEND': 'x11',
            'GPUI_X11_SCALE_FACTOR': f'{scale:g}',
            'WINIT_X11_SCALE_FACTOR': f'{scale:g}',
            'VK_DRIVER_FILES': '/usr/share/vulkan/icd.d/lvp_icd.x86_64.json',
            'LP_NUM_THREADS': '8',
            # Today's Clanky also writes its own frame log here; other builds may ignore it.
            'CLANKY_FRAME_TRACE': str(work / 'frames.csv'),
        }
        self.config = config
        self.app = None

    def start(self):
        """Launches the window; returns ms from launch to the first drawing on screen."""
        self.x.count_damage(0.2)  # anything Xvfb drew while starting
        launched = time.monotonic()
        self.launched_unix = time.time()
        self.app = subprocess.Popen(
            [str(self.binary), '--config', str(self.config)],
            env=self.env,
            cwd=self.work,
            stdout=subprocess.DEVNULL,
            stderr=(self.work / 'app.err').open('wb'),
        )
        seen = self._first_content(launched + 30)
        window = until(self._window_id, 'the window to be mapped', 15)
        subprocess.run(['xdotool', 'windowfocus', '--sync', window], env=self.env, check=True)
        until((self.work / 'shell-ready').exists, 'the stand-in shell', 15)
        until(lambda: self.x.wait_damage(0.5) is None, 'the window to settle', 15)
        return (seen - launched) * 1000

    def _first_content(self, deadline):
        """Time of the first drawing after which the screen holds more than the black root and one flat fill.

        Mapping a window paints its plain background long before the first real frame (54 ms against 340 ms
        for today's Clanky, by its own frame log), so a bare damage event is not a frame.
        """
        while (left := deadline - time.monotonic()) > 0:
            seen = self.x.wait_damage(left)
            if seen is None:
                break
            pixels = self.x.grab(0, 0, *SCREEN).view(np.uint32)
            if np.unique(pixels).size > 2:
                return seen
        raise RuntimeError('the window drew no content within 30 s')

    def _window_id(self):
        result = subprocess.run(
            ['xdotool', 'search', '--onlyvisible', '--pid', str(self.app.pid)],
            env=self.env,
            capture_output=True,
            text=True,
        )
        ids = result.stdout.split()
        return ids[0] if ids else None

    def open_scene(self):
        """Two tabs and four panes, then the palette. Returns ms from the palette's key press to its first frame."""
        for chord in SCENE_CHORDS:
            self.x.chord(*chord)
            until(lambda: self.x.wait_damage(0.5) is None, 'the layout to settle', SETTLE_SECONDS)
        self.x.count_damage(0.2)
        before = self.x.grab(0, 0, *SCREEN)
        pressed = self.x.chord(*PALETTE_CHORD)
        while True:
            seen = self.x.wait_damage(2)
            if seen is None:
                raise RuntimeError('the palette key drew nothing')
            if np.any(self.x.grab(0, 0, *SCREEN) != before):
                break
        until(lambda: self.x.wait_damage(0.5) is None, 'the palette to settle', SETTLE_SECONDS)
        return (seen - pressed) * 1000

    def latency(self, keys, rng, pair=('b', 'a')):
        """Echo latency in ms for `keys` key presses, alternating the two keys of `pair` so each changes the pixels."""
        self.x.count_damage(0.3)
        before = self.x.grab(0, 0, *SCREEN)
        self.x.key('a')
        if self.x.wait_damage(2) is None:
            raise RuntimeError('a key press drew nothing')
        time.sleep(0.5)
        box = changed_box(before, self.x.grab(0, 0, *SCREEN))
        if box is None:
            raise RuntimeError('a key press changed no pixels')
        x0, y0, x1, y1 = box
        x0, y0 = max(0, x0 - 2), max(0, y0 - 2)
        width, height = min(SCREEN[0], x1 + 2) - x0, min(SCREEN[1], y1 + 2) - y0
        samples = []
        for index in range(keys):
            self.x.count_damage(0.03 + rng.random() * 0.05)
            previous = self.x.grab(x0, y0, width, height)
            pressed = self.x.key(pair[index % 2])
            while True:
                seen = self.x.wait_damage(2)
                if seen is None:
                    raise RuntimeError(f'key {index + 1} of {keys} never showed on screen')
                if np.any(self.x.grab(x0, y0, width, height) != previous):
                    samples.append((seen - pressed) * 1000)
                    break
        return samples, box

    def cat_seconds(self):
        took = self.work / 'cat-took'
        (self.work / 'cat-go').write_text('go')
        until(took.exists, 'cat to finish', 600)
        return float(until(took.read_text, 'the cat time', 5))

    def close(self):
        pids = processes_of(self.binary)
        if self.app is not None and self.app.pid not in pids and self.app.poll() is None:
            pids.append(self.app.pid)
        stop_pids(pids)
        if self.app is not None:
            self.app.wait(timeout=10)
        self.x.close()
        stop_pids([self.xvfb.pid])
        self.xvfb.wait(timeout=10)


def trace_rows(work):
    path = work / 'frames.csv'
    return [line.split(',') for line in path.read_text().splitlines()] if path.exists() else []


def trace_frames(work):
    """Frames in the binary's own frame log, or 0 when it writes none."""
    return sum(row[0] == 'frame' for row in trace_rows(work))


def trace_first_frame_ms(work, launched_unix):
    """Launch to the first frame the binary itself logged, as a cross-check; None when it logs none."""
    rows = trace_rows(work)
    clock = next((int(row[1]) for row in rows if row[0] == 'clock'), None)
    first = next((int(row[2]) for row in rows if row[0] == 'frame'), None)
    if clock is None or first is None:
        return None
    return (clock + first) / 1000 - launched_unix * 1000


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round(share * (len(ordered) - 1)))]


def one_run(binary, scale, keys, seed, scene):
    with tempfile.TemporaryDirectory(prefix='speed-', dir=DATA_DIR) as folder:
        session = Session(binary, scale, Path(folder))
        try:
            start_ms = session.start()
            time.sleep(2)
            popover_ms = session.open_scene() if scene else None
            time.sleep(2 if scene else 0)
            memory = resident_mib(binary)
            idle_from = trace_frames(session.work)
            idle = session.x.count_damage(IDLE_SECONDS)
            idle_trace = trace_frames(session.work) - idle_from
            pair = ('BackSpace', 'a') if scene else ('b', 'a')
            samples, box = session.latency(keys, random.Random(seed), pair)
            cat = session.cat_seconds()
            trace_start = trace_first_frame_ms(session.work, session.launched_unix)
        finally:
            session.close()
    return {
        'popover_ms': popover_ms,
        'start_ms': start_ms,
        'memory_mib': memory,
        'idle_draws': idle,
        'key_p50_ms': statistics.median(samples),
        'key_p99_ms': percentile(samples, 0.99),
        'cat_s': cat,
        'echo_box': box,
        'keys': len(samples),
        'trace_start_ms': trace_start,
        'trace_idle_frames': idle_trace,
    }


POPOVER_NUMBER = ('popover_ms', 'palette first frame', 'ms', 1)
NUMBERS = [
    ('start_ms', 'start to first frame', 'ms', 1),
    ('memory_mib', 'memory after start', 'MiB', 1),
    ('idle_draws', 'drawing in 10 s idle', 'bursts', 0),
    ('key_p50_ms', 'key to screen p50', 'ms', 2),
    ('key_p99_ms', 'key to screen p99', 'ms', 2),
    ('cat_s', 'cat 100 MB', 's', 2),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('binary', type=Path)
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--keys', type=int, default=200)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--scene', choices=['palette'], help='open two tabs, four panes and the palette first')
    args = parser.parse_args()
    numbers = NUMBERS + [POPOVER_NUMBER] if args.scene else NUMBERS
    if not os.access(args.binary, os.X_OK):
        parser.error(f'{args.binary} is not an executable file')
    if args.runs < 1 or args.keys < 2:
        parser.error('--runs must be at least 1 and --keys at least 2')
    make_text_file()
    print(f'text file {TEXT_FILE}: {TEXT_FILE.stat().st_size} bytes, sha256 {sha256(TEXT_FILE)[:16]}')
    version = subprocess.run([str(args.binary), '--version'], capture_output=True, text=True).stdout.strip()
    print(f'binary {args.binary} ({version}), scale {args.scale:g}, {args.runs} runs, {args.keys} keys each')
    runs = []
    for number in range(args.runs):
        result = one_run(args.binary, args.scale, args.keys, TEXT_SEED + number, args.scene)
        runs.append(result)
        print(f'run {number + 1}: ' + ', '.join(f'{key} {result[key]:.{digits}f}' for key, _, _, digits in numbers))
        if result['trace_start_ms'] is not None:
            print(
                f'       its own frame log: first frame {result["trace_start_ms"]:.1f} ms after launch, '
                f'{result["trace_idle_frames"]} frames while idle'
            )
    print(f'{"number":<24}{"median":>10}{"spread":>22}')
    summary = {}
    for key, label, unit, digits in numbers:
        values = [run[key] for run in runs]
        summary[key] = {'median': statistics.median(values), 'low': min(values), 'high': max(values)}
        spread = f'{min(values):.{digits}f}..{max(values):.{digits}f}'
        print(f'{label:<24}{statistics.median(values):>10.{digits}f}{spread:>16} {unit}')
    if args.out:
        args.out.write_text(
            json.dumps(
                {
                    'binary': str(args.binary),
                    'version': version,
                    'scale': args.scale,
                    'scene': args.scene,
                    'runs': runs,
                    'summary': summary,
                },
                indent=2,
            )
            + '\n'
        )
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except RuntimeError as error:
        print(f'speed: {error}', file=sys.stderr)
        sys.exit(1)
