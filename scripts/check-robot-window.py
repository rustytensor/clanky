#!/usr/bin/env python3
"""M12 check 15. Capture actual X11 pixels; never call offscreen_scene.
Expected before running: antenna RGB 210,164,118 (tolerance 3); base/torso
mid-turn pixels differ from their fixed before/after samples; no frames beyond
7000/1200 ms; key x at logical 300 ms reaches the PTY and skips before one frame.
"""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time
from PIL import ImageGrab


class Failure(Exception):
    pass


def require(ok, label):
    if not ok:
        raise Failure(label)


def until(fn, label, seconds=15):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        result = fn()
        if result:
            return result
        time.sleep(.005)
    raise Failure(label)


def stop(process):
    if process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=4)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=4)


def rows(path):
    if not path.exists():
        return []
    result = []
    for line in path.read_text().splitlines():
        fields = line.split(',')
        if len(fields) >= 3:
            result.append((fields[0], int(fields[1]), int(fields[2])))
    return result


def main(work, bins):
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'robot'), 'unknown robot fault')
    target = Path(os.environ['CARGO_TARGET_DIR'])
    pictures = target / 'look/robot'
    pictures.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    for key in list(env):
        if any(word in key.upper() for word in ('KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD')):
            env.pop(key)
    for key in ('WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'CLANKY_FRAME_TRACE',
                'CLANKY_ROBOT_GATE', 'CLANKY_ROBOT_CLOCK'):
        env.pop(key, None)
    for name in ('home', 'config', 'state', 'runtime'):
        (work / name).mkdir(mode=0o700)
    env.update(HOME=str(work / 'home'), XDG_CONFIG_HOME=str(work / 'config'),
               XDG_STATE_HOME=str(work / 'state'), XDG_RUNTIME_DIR=str(work / 'runtime'),
               SHELL='/bin/sh', WINIT_UNIX_BACKEND='x11')
    config = work / 'config.json'
    config.write_text(json.dumps(dict(columns=124, rows=37, theme='dark', theme_dark='stone-dark',
                                     restore_layout=False, animations=True, startup_animation=True,
                                     osc52_write=False, osc52_read=False)))
    reader = work / 'reader.py'
    reader.write_text("import os,tty\ntty.setraw(0)\nopen('reader-ready','w').close()\n"
                      "while True:\n b=os.read(0,1)\n if not b: break\n"
                      " with open('received','ab',buffering=0) as f: f.write(b)\n")
    children, logs, traces = [], [], []
    report = {}
    socket = work / 'server.sock'

    def launch(argv, child_env, label, pipe=False):
        log = (work / (label + '.log')).open('wb')
        logs.append(log)
        process = subprocess.Popen([str(x) for x in argv], env=child_env, cwd=work,
                                   stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE if pipe else subprocess.DEVNULL, stderr=log)
        children.append(process)
        return process

    def xdo(*args):
        result = subprocess.run(['xdotool', *map(str, args)], env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=10)
        require(result.returncode == 0, 'X11 command')
        return result.stdout.decode().strip()

    class Window:
        def __init__(self, label, state='state', hello=False, duration=7000):
            self.label, self.hello = label, hello
            self.trace = work / (label + '.csv')
            self.clock = work / (label + '.clock')
            self.clock.write_text('0\n')
            traces.append(self.trace)
            child_env = dict(env, CLANKY_FRAME_TRACE=str(self.trace), CLANKY_ROBOT_GATE='1',
                             CLANKY_ROBOT_CLOCK=str(self.clock), XDG_STATE_HOME=str(work / state))
            argv = [bins / 'clanky', '+hello'] if hello else [bins / 'clanky', '--socket', socket]
            self.process = launch([*argv, '--config', config], child_env, label)
            until(lambda: any(r[0] == 'gate_frame' for r in rows(self.trace)), label + ' initial frame')
            self.wid = xdo('search', '--onlyvisible', '--pid', self.process.pid).splitlines()[0]
            xdo('windowsize', self.wid, 448 if hello else 1240, 448 if hello else 760)
            xdo('windowmove', self.wid, 0, 0)
            xdo('windowfocus', '--sync', self.wid)
            # Finish real startup housekeeping while the animation clock stays
            # at zero. Accelerating seven seconds must not outrun initial PTY,
            # settings or focus notifications and misclassify those as animation.
            settled = time.monotonic()
            count = len(self.frames())
            deadline = settled + 12
            while time.monotonic() - settled < 2:
                require(time.monotonic() < deadline, label + ' startup settles at clock zero')
                time.sleep(.02)
                current = len(self.frames())
                if current != count:
                    count, settled = current, time.monotonic()
            if not hello:
                starts = [r for r in rows(self.trace) if r[0] == 'robot_start']
                require(len(starts) == 1 and starts[0][1] == duration, label + ' first/later greeting')
            self.duration = duration

        def frames(self):
            return [r for r in rows(self.trace) if r[0] == 'gate_frame']

        def set_time(self, ms):
            temporary = self.clock.with_suffix('.next')
            temporary.write_text(str(ms) + '\n')
            temporary.replace(self.clock)

        def advance(self, ms):
            before = len(self.frames())
            self.set_time(ms)
            until(lambda: len(self.frames()) > before and self.frames()[-1][1] == ms,
                  self.label + ' frame at ' + str(ms))

        def zero_after(self, end):
            before = len(self.frames())
            # Drive the same FrameClock through the next two logical seconds.
            for ms in (end + 16, end + 500, end + 1000, end + 1500, end + 2000):
                self.set_time(ms)
                time.sleep(.03)
            time.sleep(2)  # also prove the real window remains quiet
            require(self.process.poll() is None, self.label + ' window alive')
            extra = len(self.frames()) - before
            require(extra == 0, f'{self.label} zero frames after {end} ms: observed {extra}')
            require(all(r[1] <= end for r in self.frames()), self.label + ' frame time bound')
            return len(self.frames())

        def capture(self, ms):
            self.advance(ms)
            time.sleep(.15)  # GPU presentation plus an X round trip, not an animation sleep
            geometry = dict(line.split('=', 1) for line in xdo('getwindowgeometry', '--shell', self.wid).splitlines())
            x, y, w, h = (int(geometry[k]) for k in ('X', 'Y', 'WIDTH', 'HEIGHT'))
            require((w, h) == (448, 448), 'fixed replay window size')
            image = ImageGrab.grab(bbox=(x, y, x + w, y + h), xdisplay=env['DISPLAY']).convert('RGB')
            image.save(pictures / f'frame-{ms}.png')
            return image

    try:
        launch([bins / 'clanky', '--server', '--socket', socket], env, 'server')
        until(lambda: socket.exists(), 'private server startup')
        owner = launch([bins / 'clanky-agent-gate', 'owner', socket], env, 'owner', pipe=True)

        def call(op, args=None):
            owner.stdin.write(json.dumps(dict(v=1, op=op, args=args or {})).encode() + b'\n')
            owner.stdin.flush()
            require(select.select([owner.stdout], [], [], 15)[0], 'owner reply deadline')
            reply = json.loads(owner.stdout.readline())
            require(reply.get('ok'), 'owner RPC ' + op)
            return reply['result']

        tab = call('open_tab', dict(rows=24, cols=80, allow_agent=False))['tab_id']
        call('run', dict(tab_id=tab, argv=[sys.executable, str(reader)], cwd=str(work)))
        until(lambda: (work / 'reader-ready').exists(), 'PTY reader startup')

        first = Window('first')
        for ms in (16, 300, 1200, 3200, 5850, 6999, 7000):
            first.advance(ms)
        require(any(r[0] == 'robot_end' for r in rows(first.trace)), 'full scene ended')
        report['first_frames'] = first.zero_after(7000)
        stop(first.process)
        marker = work / 'state/clanky/first-meeting'
        require(marker.read_bytes() == b'Clanky first meeting 1\n', 'first-run marker')

        later = Window('later', duration=1200)
        for ms in (16, 300, 1199, 1200):
            later.advance(ms)
        report['later_frames'] = later.zero_after(1200)
        stop(later.process)

        key = Window('key', state='state-key')
        key.advance(300)
        before = len(key.frames())
        xdo('key', '--window', key.wid, 'x')
        until(lambda: (work / 'received').exists(), 'key reaches real PTY')
        require((work / 'received').read_bytes() == b'x', 'exact key byte at 300 ms')
        skipped = until(lambda: next((r for r in rows(key.trace) if r[0] == 'robot_skip'), None), 'key skip')
        until(lambda: len(key.frames()) > before, 'skip clear frame')
        require(key.frames()[before][1] == 300, 'skip clears in next frame at 300 ms')
        require(not any(r[0] == 'robot_sample' and r[2] > skipped[2] for r in rows(key.trace)), 'no overlay after key')
        key.zero_after(300)
        report['key_at_ms'] = 300
        report['key_bytes'] = 'x'
        stop(key.process)

        # Normal startup clears its overlay at 7000. +hello deliberately retains
        # that final pose and uses the same robot builder and real surface renderer.
        replay = Window('pixels', hello=True)
        images = {ms: replay.capture(ms) for ms in (2400, 3200, 4000, 5400, 5850, 6300, 7000)}
        # Replay is 448x448: 24px padding leaves a 400x400 reference canvas.
        # Antenna: (200,62), -7deg about (200,130), then 1.07 about (200,260),
        # plus padding -> (215.133,72.682); use its nearest centre pixel (215,73).
        antenna = images[7000].getpixel((215, 73))
        require(all(abs(a - b) <= 3 for a, b in zip(antenna, (210, 164, 118))),
                f'antenna expected (210,164,118), observed {antenna}')
        base = [images[ms].getpixel((194, 324)) for ms in (5400, 5850, 6300)]
        torso = [images[ms].getpixel((202, 229)) for ms in (2400, 3200, 4000)]
        require(base[1] != base[0] and base[1] != base[2], '5850 ms base differs from 5400/6300')
        require(torso[1] != torso[0] and torso[1] != torso[2], '3200 ms torso differs from 2400/4000')
        report.update(antenna_rgb=antenna, base_rgb=base, torso_rgb=torso)
        stop(replay.process)
        for entry in call('list_tabs')['tabs']:
            call('close_tab', dict(tab_id=entry['tab_id']))
        (target / 'robot-window-result.json').write_text(json.dumps(report, indent=2) + '\n')
        print('PASS: M12 robot real-window check 15 (clock, key, pixels, first-run marker)')
    finally:
        for process in reversed(children):
            stop(process)
        for log in logs:
            log.close()
        for trace in traces:
            if trace.exists():
                (target / ('robot-window-' + trace.name)).write_bytes(trace.read_bytes())


if __name__ == '__main__':
    try:
        main(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
    except Failure as error:
        print('FAIL: M12 robot ' + str(error), file=sys.stderr)
        sys.exit(1)
    except BaseException:
        print('FAIL: M12 robot fixture or transport', file=sys.stderr)
        sys.exit(1)
