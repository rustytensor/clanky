#!/usr/bin/env python3
"""Real X11 input into the real window, with a private server, home and state.
Only synthetic x and pointer coordinates are injected. No capability is printed.
"""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time


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
        time.sleep(.01)
    raise Failure(label)


def stop(process):
    if process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=4)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=4)


def main(work, bins):
    env = os.environ.copy()
    for key in list(env):
        if any(word in key.upper() for word in ('KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD')):
            env.pop(key)
    for key in ('WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'CLANKY_FRAME_TRACE'):
        env.pop(key, None)
    for name in ('home', 'config', 'state', 'runtime'):
        (work / name).mkdir(mode=0o700)
    env.update(HOME=str(work / 'home'), XDG_CONFIG_HOME=str(work / 'config'),
               XDG_STATE_HOME=str(work / 'state'), XDG_RUNTIME_DIR=str(work / 'runtime'),
               SHELL='/bin/sh', WINIT_UNIX_BACKEND='x11')
    config = work / 'config.json'
    config.write_text(json.dumps(dict(columns=124, rows=37, theme='dark',
                                     restore_layout=False, animations=True, startup_animation=True,
                                     osc52_write=False, osc52_read=False)))
    reader = work / 'reader.py'
    reader.write_text("import os,tty\ntty.setraw(0)\nopen('reader-ready','w').close()\n"
                      "while True:\n b=os.read(0,1)\n if not b: break\n"
                      " with open('received','ab',buffering=0) as f: f.write(b)\n")
    children, logs, traces = [], [], []
    socket = work / 'server.sock'

    def launch(argv, child_env, pipe=False, label='process'):
        log = (work / (label + '.log')).open('wb')
        logs.append(log)
        process = subprocess.Popen([str(x) for x in argv], env=child_env, cwd=work,
                                   stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE if pipe else subprocess.DEVNULL,
                                   stderr=log)
        children.append(process)
        return process

    owner = None
    summary = {}
    try:
        server = launch([bins / 'clanky', '--server', '--socket', socket], env, label='server')
        until(lambda: socket.exists(), 'private server startup')
        owner = launch([bins / 'clanky-agent-gate', 'owner', socket], env, pipe=True, label='owner')

        def call(op, args=None, version=1):
            owner.stdin.write(json.dumps(dict(v=version, op=op, args=args or {})).encode() + b'\n')
            owner.stdin.flush()
            require(select.select([owner.stdout], [], [], 15)[0], 'owner reply deadline')
            reply = json.loads(owner.stdout.readline())
            require(reply.get('ok'), 'owner RPC ' + op)
            return reply['result']

        tab = call('open_tab', dict(rows=24, cols=80, allow_agent=False))['tab_id']
        call('run', dict(tab_id=tab, argv=[sys.executable, str(reader)], cwd=str(work)))
        until(lambda: (work / 'reader-ready').exists(), 'PTY reader startup')

        def rows(path):
            if not path.exists():
                return []
            result = []
            for line in path.read_text().splitlines():
                fields = line.split(',')
                if len(fields) >= 3:
                    result.append((fields[0], int(fields[1]), int(fields[2])))
            return result

        def first(path, kind):
            return next((row for row in rows(path) if row[0] == kind), None)

        def frame_count(path):
            return sum(row[0] == 'frame' for row in rows(path))

        def xdo(*args):
            result = subprocess.run(['xdotool', *map(str, args)], env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=10)
            require(result.returncode == 0, 'X11 input command')
            return result.stdout.decode().strip()

        def window(label, state='state', expected=7000):
            trace = work / (label + '.csv')
            traces.append(trace)
            child_env = dict(env, CLANKY_FRAME_TRACE=str(trace), XDG_STATE_HOME=str(work / state))
            app = launch([bins / 'clanky', '--socket', socket, '--config', config], child_env, label=label)
            until(lambda: first(trace, 'robot_start'), label + ' robot start')
            require(first(trace, 'robot_start')[1] == expected, label + ' greeting kind')
            until(lambda: frame_count(trace) > 0, label + ' first frame')
            wid = xdo('search', '--onlyvisible', '--pid', app.pid).splitlines()[0]
            xdo('windowsize', wid, 1240, 760)
            xdo('windowfocus', '--sync', wid)
            return app, trace, wid

        def idle_after_end(app, trace, duration):
            ended = until(lambda: first(trace, 'robot_end'), 'robot end deadline', 12)
            until(lambda: any(row[0] == 'frame' and row[2] >= ended[2] for row in rows(trace)),
                  'final clear frame')
            # Exclude queued X resize/focus events; the measured interval below is
            # two full quiet seconds after the actual final clear frame.
            time.sleep(.2)
            count = frame_count(trace)
            time.sleep(2)
            require(app.poll() is None, 'window remained alive')
            require(frame_count(trace) == count, 'zero frames after robot')
            samples = [row for row in rows(trace) if row[0] == 'robot_sample']
            require(samples and all(0 <= row[1] < duration for row in samples), 'bounded robot clock')
            require(not any(row[0] == 'robot_sample' and row[2] > ended[2] for row in rows(trace)),
                    'no samples after end')
            return count

        app, trace, _ = window('first')
        summary['first_frames'] = idle_after_end(app, trace, 7000)
        stop(app)
        marker = work / 'state/clanky/first-meeting'
        require(marker.read_bytes() == b'Clanky first meeting 1\n', 'first-run marker bytes')
        app, trace, _ = window('later', expected=1200)
        summary['later_frames'] = idle_after_end(app, trace, 1200)
        stop(app)

        app, trace, wid = window('key', state='state-key')
        time.sleep(.3)
        xdo('key', '--window', wid, 'x')
        until(lambda: (work / 'received').exists(), 'key reaches real PTY')
        require((work / 'received').read_bytes() == b'x', 'exact PTY key byte')
        skipped = until(lambda: first(trace, 'robot_skip'), 'key skips robot')
        until(lambda: any(row[0] == 'frame' and row[2] >= skipped[2] for row in rows(trace)),
              'key clear frame')
        time.sleep(.1)
        require(not any(row[0] == 'robot_sample' and row[2] > skipped[2] for row in rows(trace)),
                'key skip stops overlay before next frame')
        summary['key_byte'] = 'x'
        stop(app)

        before = len(call('list_tabs')['tabs'])
        app, trace, wid = window('click', state='state-click')
        time.sleep(.3)
        # The existing app's New tab button is right-aligned in the 1240px top bar.
        xdo('mousemove', '--window', wid, 1180, 36)
        xdo('click', '1')
        until(lambda: len(call('list_tabs')['tabs']) == before + 1, 'click reaches New tab')
        skipped = until(lambda: first(trace, 'robot_skip'), 'click skips robot')
        require(not any(row[0] == 'robot_sample' and row[2] > skipped[2] for row in rows(trace)),
                'click skip stops overlay')
        summary['new_tabs'] = 1
        stop(app)
        for entry in call('list_tabs')['tabs']:
            call('close_tab', dict(tab_id=entry['tab_id']))
        print('PASS: real windows full then short; zero idle frames; key x reached PTY; click opened one tab')
        report = Path(os.environ['CARGO_TARGET_DIR']) / 'robot-startup-result.json'
        report.write_text(json.dumps(summary, indent=2) + '\n')
    finally:
        for process in reversed(children):
            stop(process)
        for log in logs:
            log.close()
        # Bounded diagnostics hold only synthetic test-window timestamps/counts.
        target = Path(os.environ['CARGO_TARGET_DIR'])
        for trace in traces:
            if trace.exists():
                (target / ('robot-startup-' + trace.name)).write_bytes(trace.read_bytes())


if __name__ == '__main__':
    try:
        main(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
    except Failure as error:
        print('FAIL: robot startup ' + str(error), file=sys.stderr)
        sys.exit(1)
    except BaseException:
        print('FAIL: robot startup fixture or transport', file=sys.stderr)
        sys.exit(1)
