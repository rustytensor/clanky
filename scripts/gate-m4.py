#!/usr/bin/env python3
"""Fixed M4 real-process expectations. Never prints credentials or requests."""
import base64
import fcntl
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

RAW_A = b'\x1b]133;C\x07one\r\n\x1b]133;D;0\x07h0\r\nh1\r\nh2\r\nh3\r\nh4\r\n'
PREFIX_B = b'\x1b]133;C\x07start\r\n'
RAW_B = PREFIX_B + b'done\r\n\x1b]133;D;7\x07'
PROBE = dict(auth_code='permission_denied', read_code='permission_denied',
             auth_denied=True, read_denied=True, bridge_continued=True)


def require(value, message):
    if not value:
        raise AssertionError(message)


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def gone(pid, seconds, label):
    deadline = time.monotonic() + seconds
    while alive(pid):
        require(time.monotonic() < deadline, label + ': PID still alive')
        time.sleep(0.02)


def stop_child(child):
    if child is None:
        return
    if child.poll() is None:
        child.terminate()
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait(timeout=5)


def stop_pid(pid):
    # Never turn a malformed fixture PID into a process-group signal.
    if type(pid) is not int or pid <= 1 or pid == os.getpid() or not alive(pid):
        return
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + 5
    while alive(pid) and time.monotonic() < deadline:
        time.sleep(0.02)
    if alive(pid):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    gone(pid, 5, 'fixture cleanup')


def private_write(path, data):
    with open(path, 'x', opener=lambda p, f: os.open(p, f, 0o600)) as file:
        file.write(data)


def raw(value, expected, eof, label):
    require(value.get('encoding') == 'base64', label + ': encoding')
    require(base64.b64decode(value['data'], validate=True) == expected, label + ': exact raw bytes')
    require(value.get('truncated') is False and value.get('eof') is eof, label + ': drain state')
    require(value.get('retained_start') == 0 and value.get('next_offset') == len(expected)
            and value.get('total_bytes') == len(expected), label + ': raw offsets')


def status(value, code, label):
    require(value == dict(done=True, state='exited', exit_code=code, signal=None), label + ': actual exit')


def persisted(o, seed, label):
    a, b = seed['a'], seed['b']
    require(o['hello'].get('version') == 3 and o['hello'].get('server_pid') == seed['hello']['server_pid'],
            label + ': same actual server PID')
    require(o['hello'].get('capabilities') == ['sessions', 'owner'], label + ': owner capabilities')
    expected_groups = [dict(layout={'Split': dict(axis='Columns', first={'Leaf': a}, second={'Leaf': b})}, focus=b)]
    require(o['workspace'].get('groups') == expected_groups and o['workspace'].get('active') == 0,
            label + ': exact split/focus/active')
    require(o['workspace'] == seed['workspace'], label + ': unchanged workspace revision')
    require(o['permission_a'] is False and o['permission_b'] is True, label + ': permissions')
    raw(o['raw_a'], RAW_A, True, label + ' A')
    require(o['history'] == dict(total=4, offset=0, next_offset=4, lines=['one', 'h0', 'h1', 'h2']),
            label + ': exact retained history')
    require(o['screen'] == dict(rows=3, cols=20, lines=['one', 'h0', 'h1'], cursor=None),
            label + ': oldest scrolled screen')
    view = o['view_a']
    require(view.get('has_more') is False and len(view['blocks']) == 1, label + ': A block count')
    block = view['blocks'][0]
    require(block.get('output') == 'one', label + ': A block output expected one')
    require(view == seed['view_a'], label + ': unchanged A block/action metadata')
    require(block.get('block_id') == seed['view_a']['blocks'][0]['block_id']
            and block.get('command_id') == seed['ca'] and block.get('complete') is True
            and block.get('truncated') is False and block.get('exit_code') == 0
            and block.get('collapsed') is True, label + ': A block identity/state')
    require(view.get('selection') == dict(block_id=block['block_id'], start=0, end=3), label + ': exact selection')
    task = o['task']
    require(task == seed['task'], label + ': retained task')
    require(task.get('task_id') and task.get('label') == 'Persist' and task.get('state') == 'queued'
            and task.get('tab_id') == a and task.get('command_id') == seed['ca']
            and task.get('block_id') == block['block_id'] and task.get('encoding') == 'base64'
            and task.get('data') == 'b25l' and task.get('bytes') == 3, label + ': exact task payload/source')
    status(task['process'], 0, label + ' task process')
    require(view.get('tasks') == [{k: v for k, v in task.items() if k not in ('encoding', 'data', 'bytes')}],
            label + ': retained task list')


def denied_agent(path, seed):
    # Independent unowned v1 stream; no credential is ever read by Python.
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(10)
        sock.connect(str(path))
        request = dict(v=1, id='1', op='get_output', tab_id=seed['a'], command_id=seed['ca'],
                       offset=0, limit=64, encoding='base64')
        sock.sendall(json.dumps(request).encode() + b'\n')
        data = bytearray()
        while b'\n' not in data:
            chunk = sock.recv(4096)
            require(chunk, 'unowned v1 disconnected')
            data.extend(chunk)
            require(len(data) <= 65536, 'unowned v1 oversized response')
        reply = json.loads(data.partition(b'\n')[0])
        require(reply.get('id') == '1' and reply.get('ok') is False
                and reply.get('error', {}).get('code') == 'permission_denied', 'unowned v1 A denial')


def read_ready(child, output, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        output.flush()
        with open(output.name, 'rb') as reader:
            data = reader.read(2 * 1024 * 1024 + 1)
        require(len(data) <= 2 * 1024 * 1024, 'helper observations oversized')
        if b'\n' in data:
            return json.loads(data)
        require(child.poll() is None, 'seed helper exited before readiness')
        time.sleep(0.02)
    raise TimeoutError('seed helper exceeded 30 seconds')


def fixture(target):
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'reattach'), 'unknown CLANKY_GATE_FAULT')
    with tempfile.TemporaryDirectory(prefix='clanky-m4-', dir=target) as directory:
        root = Path(directory)
        root.chmod(0o700)
        debug = root / 'debug'
        debug.mkdir(mode=0o700)
        # Snapshot every required executable under the single shared build lock.
        # No compiler or gate is launched by this capture.
        names = ['clanky', 'clanky-session-gate']
        if not fault:
            names += ['clanky-actions-gate', 'clanky-state-gate', 'clanky-image-gate', 'clanky-text-gate']
        lock_path = Path(os.environ.get('CLANKY_BUILD_LOCK', str(target / '.build.lock')))
        with open(lock_path, 'a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            for name in names:
                binary = target / 'debug' / name
                require(binary.is_file() and os.access(binary, os.X_OK), 'missing gate executable: ' + name)
                shutil.copyfile(binary, debug / name)
                (debug / name).chmod(0o700)
        helper = debug / 'clanky-session-gate'
        path = root / 'server.sock'
        private_write(root / 'a.sh', "printf '\\033]133;C\\007one\\n\\033]133;D;0\\007h0\\nh1\\nh2\\nh3\\nh4\\n'\n")
        seed_child = inspect_child = previous = None
        server_pid = b_pid = None
        try:
            with open(root / 'seed.out', 'x+', opener=lambda p, f: os.open(p, f, 0o600)) as out, \
                    open(root / 'helper.log', 'x+', opener=lambda p, f: os.open(p, f, 0o600)) as log:
                seed_child = subprocess.Popen([str(helper), '--seed', '--socket', str(path), '--dir', str(root)],
                                              stdin=subprocess.PIPE, stdout=out, stderr=log)
                seed = read_ready(seed_child, out, 30)
                server_pid = seed['hello']['server_pid']
                require(isinstance(server_pid, int) and server_pid > 1 and server_pid != seed_child.pid, 'actual server PID')
                b_pid = int((root / 'b.pid').read_text())
                require(b_pid > 1 and b_pid not in (server_pid, seed_child.pid), 'distinct B PID')
                require(seed['a'] and seed['b'] and seed['a'] != seed['b'] and seed['ca'] and seed['cb'], 'distinct saved tab IDs')
                require(seed['probe'] == PROBE, 'inside-B real-token auth/read denial and continuing bridge')
                persisted(seed, seed, 'before detach')
                status(seed['status_a'], 0, 'A')
                raw(seed['raw_b'], PREFIX_B, False, 'B before detach')
                require(seed['status_b'].get('done') is False and seed['status_b'].get('state') == 'running', 'B running before detach')
                require(alive(b_pid) and alive(server_pid), 'B/server alive before detach')
                denied_agent(path, seed)
                private_write(root / 'seed.json', json.dumps(seed))
                # Actual abrupt process loss, not a Client drop or a fake detach.
                seed_child.kill()
                require(seed_child.wait(timeout=5) == -signal.SIGKILL, 'seed SIGKILL')
                gone(seed_child.pid, 5, 'seed client')
                require(alive(server_pid), 'same server survives client SIGKILL')
                private_write(root / 'release', '')
                gone(b_pid, 10, 'B completion while detached')
                require(alive(server_pid), 'server alive after detached B exit')
                with open(root / 'inspect.out', 'x+', opener=lambda p, f: os.open(p, f, 0o600)) as observed:
                    inspect_child = subprocess.Popen([str(helper), '--inspect', '--socket', str(path), '--dir', str(root)],
                                                     stdout=observed, stderr=log)
                    require(inspect_child.wait(timeout=30) == 0, 'stdio inspection helper failed')
                    observed.seek(0)
                    data = observed.read(2 * 1024 * 1024 + 1)
                    require(len(data) <= 2 * 1024 * 1024, 'inspection observations oversized')
                    o = json.loads(data)
                # The sole fault mutation: only observed A output changes.
                if fault:
                    o['view_a']['blocks'][0]['output'] = 'lost'
                persisted(o, seed, 'reattached')
                require(sorted(o['tabs']) == sorted([seed['a'], seed['b']]), 'same two terminal IDs, no recreated shell')
                require(o['agent_a_denied'] is True, 'reattached unowned v1 A permission_denied')
                status(o['status_a'], 0, 'reattached A')
                status(o['status_b'], 7, 'detached B')
                raw(o['raw_b'], RAW_B, True, 'reattached B')
                view = o['view_b']
                require(view.get('has_more') is False and len(view['blocks']) == 1, 'B completed block count')
                block = view['blocks'][0]
                require(block.get('block_id') == seed['view_b']['blocks'][0]['block_id']
                        and block.get('command_id') == seed['cb'] and block.get('output') == 'start\ndone'
                        and block.get('complete') is True and block.get('exit_code') == 7
                        and block.get('truncated') is False, 'B exact completed block and command ID')
                gone(server_pid, 5, 'last close exits server')
                print('M4 persistence and inside-terminal owner denial PASS', flush=True)
            if not fault:
                env = dict(os.environ, CARGO_TARGET_DIR=str(root))
                previous = subprocess.Popen([str(Path(__file__).resolve().with_name('gate-m3.sh'))], env=env)
                require(previous.wait(timeout=240) == 0, 'M3 regression failed (includes M2/M1)')
        finally:
            stop_child(inspect_child)
            stop_child(seed_child)
            stop_child(previous)
            # Recover only fixture-owned saved PIDs even if seed failed before
            # printing readiness. Never inspect or kill by process-name pattern.
            if server_pid is None and (root / 'server.json').exists():
                server_pid = json.loads((root / 'server.json').read_text()).get('server_pid')
            if server_pid is None and (root / 'launcher.json').exists():
                server_pid = json.loads((root / 'launcher.json').read_text()).get('server_pid')
            if b_pid is None and (root / 'b.pid').exists():
                b_pid = int((root / 'b.pid').read_text())
            stop_pid(server_pid)
            stop_pid(b_pid)
            if seed_child is not None and seed_child.stdin is not None:
                seed_child.stdin.close()


def interrupted(signum, frame):
    raise KeyboardInterrupt()


if __name__ == '__main__':
    os.umask(0o077)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        fixture(Path(os.environ.get('CARGO_TARGET_DIR', str(Path(__file__).resolve().parents[1] / 'target'))).resolve())
    except (Exception, KeyboardInterrupt) as exc:
        # Exceptions use fixed labels only; never dump helper logs or requests.
        message = str(exc) if isinstance(exc, (AssertionError, TimeoutError)) else type(exc).__name__
        print('FAIL: ' + message, flush=True)
        sys.exit(1)
    print('PASS', flush=True)
