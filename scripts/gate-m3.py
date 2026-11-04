#!/usr/bin/env python3
"""M3 real-PTY observations compared with the pre-implementation design constants."""
import base64
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time

RAW = b'\x1b]133;C\x07one\r\n\x1b]133;D;0\x07\x1b]133;C\x07two\r\n\x1b]133;D;0\x07'
ACTIONS = ['select', 'copy', 'collapse', 'rerun', 'send', 'task_update',
           'view', 'tasks', 'task_get']


def require(condition, message):
    if not condition:
        raise AssertionError(message)


class Client:
    def __init__(self, path, timeout):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(timeout)
        try:
            self.sock.connect(str(path))
        except BaseException:
            self.sock.close()
            raise
        self.pending = bytearray()
        self.serial = 0

    def close(self):
        self.sock.close()

    def exchange(self, version, op, deadline=None, **args):
        deadline = deadline or time.monotonic() + 10
        self.serial += 1
        identity = str(self.serial)
        self.sock.settimeout(max(0.001, deadline - time.monotonic()))
        self.sock.sendall(json.dumps(dict(v=version, id=identity, op=op, **args)).encode() + b'\n')
        while b'\n' not in self.pending:
            require(time.monotonic() < deadline, op + ': response deadline')
            self.sock.settimeout(max(0.001, deadline - time.monotonic()))
            data = self.sock.recv(65536)
            require(data, op + ': disconnected')
            self.pending.extend(data)
            require(len(self.pending) <= 2 * 1024 * 1024, op + ': oversized response')
        line, _, tail = self.pending.partition(b'\n')
        self.pending = bytearray(tail)
        reply = json.loads(line)
        require(reply.get('id') == identity, op + ': wrong response id')
        return reply

    def call(self, version, op, **args):
        reply = self.exchange(version, op, **args)
        require(reply.get('ok') is True, op + ': ' + repr(reply.get('error')))
        return reply['result']


def wait(client, tab, command):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        result = client.call(1, 'wait', deadline=deadline, tab_id=tab,
                             command_id=command, timeout_ms=1000)
        if result.get('done') is True:
            return result
    raise TimeoutError('command completion exceeded 10 seconds')


def observe(client, transcript):
    o = {}
    o['actions'] = client.call(2, 'actions')
    tab = client.call(1, 'open_tab', rows=24, cols=80, allow_agent=True)['tab_id']
    command = client.call(1, 'run', tab_id=tab, argv=['sh', str(transcript)],
                          cwd=str(transcript.parent))['command_id']
    o.update(tab_id=tab, command_id=command, status=wait(client, tab, command))

    def action(op, **args):
        return client.call(2, op, tab_id=tab, **args)

    def raw():
        return client.call(1, 'get_output', tab_id=tab, command_id=command,
                           offset=0, limit=65536, encoding='base64')

    o['initial'] = action('view')
    o['raw_initial'] = raw()
    block = o['initial']['blocks'][0]['block_id']
    o['selection'] = action('select', block_id=block, start=0, end=3)
    o['copy'] = action('copy', block_id=block)
    o['collapse'] = action('collapse', block_id=block, collapsed=True)
    o['collapsed'] = action('view')
    o['raw_collapsed'] = raw()
    o['expand'] = action('collapse', block_id=block, collapsed=False)
    o['expanded'] = action('view')
    o['tabs_before'] = client.call(1, 'list_tabs')
    o['missing_argv'] = client.exchange(2, 'rerun', tab_id=tab, block_id=block,
                                       cwd=str(transcript.parent))
    o['tabs_after'] = client.call(1, 'list_tabs')
    o['rerun'] = action('rerun', block_id=block, argv=['sh', '-c', 'printf again'],
                        cwd=str(transcript.parent))
    new = o['rerun']
    o['rerun_status'] = wait(client, new['tab_id'], new['command_id'])
    o['rerun_output'] = client.call(1, 'get_output', tab_id=new['tab_id'],
                                    command_id=new['command_id'], offset=0,
                                    limit=65536, encoding='base64')
    o['after_rerun'] = action('view')
    o['raw_after_rerun'] = raw()
    o['queued'] = action('send', block_id=block, label='Review first')
    task = o['queued']['task_id']
    o['running'] = action('task_update', task_id=task, state='running')
    o['done'] = action('task_update', task_id=task, state='done')
    o['tasks'] = action('tasks')
    o['task_get'] = action('task_get', task_id=task)
    denied = client.call(1, 'open_tab', allow_agent=False)['tab_id']
    o['denied'] = client.exchange(2, 'view', tab_id=denied)
    return o


def status(value, label):
    require(value == dict(done=True, state='exited', exit_code=0, signal=None),
            label + ': expected actually exited 0 / observed ' + repr(value))


def raw_output(value, expected, label):
    require(value.get('encoding') == 'base64', label + ': encoding')
    require(base64.b64decode(value['data'], validate=True) == expected, label + ': exact bytes')
    require(value.get('truncated') is False and value.get('eof') is True,
            label + ': truncated or undrained')
    require(value.get('retained_start') == 0 and value.get('next_offset') == len(expected)
            and value.get('total_bytes') == len(expected), label + ': offsets')


def check(o, label):
    require(sorted(o['actions']['actions']) == sorted(ACTIONS), label + ': actions')
    tab, command = o['tab_id'], o['command_id']
    require(isinstance(tab, str) and bool(tab) and isinstance(command, str) and bool(command), label + ': source IDs')
    status(o['status'], label + ': source')
    first = o['initial']['blocks'][0]['block_id']
    second = o['initial']['blocks'][1]['block_id']
    require(bool(first) and bool(second) and first != second, label + ': distinct block IDs')
    selection = dict(block_id=first, start=0, end=3)
    require(o['selection'] == dict(selection=selection), label + ': exact selection')
    # The planted fault changes ONLY this observed payload, never this expectation.
    require(o['copy'].get('data') == 'b25l', label + ': copy expected b25l / observed ' + repr(o['copy'].get('data')))
    require(o['copy'] == dict(encoding='base64', data='b25l', bytes=3), label + ': copy fields')
    require(base64.b64decode(o['copy']['data'], validate=True) == b'one', label + ': copy text')
    for key, collapsed in [('initial', False), ('collapsed', True), ('expanded', False), ('after_rerun', False)]:
        view = o[key]
        require(view.get('has_more') is False, label + ': view pagination')
        require(view.get('selection') == (None if key == 'initial' else selection), label + ': retained selection')
        blocks = view['blocks']
        require(len(blocks) == 2, label + ': exactly two blocks')
        for b, identity, text, flag in zip(blocks, (first, second), ('one', 'two'), (collapsed, False)):
            require(b.get('block_id') == identity and b.get('command_id') == command,
                    label + ': retained block identity')
            require(b.get('output') == text and b.get('complete') is True
                    and b.get('truncated') is False and b.get('exit_code') == 0,
                    label + ': exact completed block')
            require(b.get('collapsed') is flag, label + ': collapse flag')
        # All existing block fields, not just output, survive view-only changes.
        require([{k: v for k, v in b.items() if k != 'collapsed'} for b in blocks]
                == [{k: v for k, v in b.items() if k != 'collapsed'} for b in o['initial']['blocks']],
                label + ': original block data changed')
    require(o['collapse'] == dict(block_id=first, collapsed=True)
            and o['expand'] == dict(block_id=first, collapsed=False), label + ': collapse results')
    for key in ('raw_initial', 'raw_collapsed', 'raw_after_rerun'):
        raw_output(o[key], RAW, label + ': ' + key)
    require(o['missing_argv'].get('ok') is False
            and o['missing_argv']['error']['code'] == 'invalid_request', label + ': missing argv')
    require(o['tabs_before'] == o['tabs_after'], label + ': invalid rerun created terminal')
    new = o['rerun']
    require(new.get('source_tab_id') == tab and new.get('block_id') == first,
            label + ': rerun source')
    require(bool(new.get('tab_id')) and new['tab_id'] != tab
            and bool(new.get('command_id')) and new['command_id'] != command,
            label + ': rerun must create new terminal and command')
    status(o['rerun_status'], label + ': rerun')
    raw_output(o['rerun_output'], b'again', label + ': rerun output')
    task_id = o['queued']['task_id']
    require(isinstance(task_id, str) and bool(task_id), label + ': task ID')
    for key in ('queued', 'running', 'done', 'task_get'):
        task = o[key]
        require(task.get('task_id') == task_id and task.get('label') == 'Review first'
                and task.get('tab_id') == tab and task.get('block_id') == first
                and task.get('command_id') == command, label + ': task source')
        require(task.get('state') == ('done' if key == 'task_get' else key), label + ': claimed task state')
        status(task.get('process'), label + ': task process ' + key)
    for key in ('queued', 'task_get'):
        task = o[key]
        require(all(task.get(k) == v for k, v in dict(encoding='base64', data='b25l', bytes=3).items()),
                label + ': exact task payload')
    listed = o['tasks']['tasks']
    require(len(listed) == 1, label + ': task list size')
    metadata = {k: v for k, v in o['task_get'].items() if k not in ('encoding', 'data', 'bytes')}
    require(listed[0] == metadata, label + ': task list metadata only')
    require(o['denied'].get('ok') is False and o['denied']['error']['code'] == 'permission_denied',
            label + ': denied terminal')


def stop(process, helper=False):
    if process is not None:
        if helper and process.poll() is None:
            # Give the helper's 25-second Engine shutdown watchdog time to
            # reap its PTYs, even if this Python process was interrupted.
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                pass
        if process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def gate():
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'copy'), 'unknown CLANKY_GATE_FAULT')
    target = Path(os.environ.get('CARGO_TARGET_DIR', str(Path(__file__).resolve().parents[1] / 'target'))).resolve()
    app, helper = (target / 'debug' / name for name in ('clanky', 'clanky-actions-gate'))
    for binary in (app, helper):
        require(binary.is_file() and os.access(binary, os.X_OK), 'missing executable: ' + str(binary))
    with tempfile.TemporaryDirectory(prefix='clanky-m3-', dir=target) as directory:
        root = Path(directory)
        os.chmod(root, 0o700)
        transcript = root / 'transcript.sh'
        transcript.write_text("printf '\\033]133;C\\007'\nprintf 'one\\n'\nprintf '\\033]133;D;0\\007'\n"
                              "printf '\\033]133;C\\007'\nprintf 'two\\n'\nprintf '\\033]133;D;0\\007'\n")
        transcript.chmod(0o600)
        client = process = None
        with tempfile.TemporaryFile(dir=root) as log:
            try:
                path = root / 'agent.sock'
                process = subprocess.Popen([str(app), '--headless', '--socket', str(path)], stdout=log, stderr=log)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    require(process.poll() is None, 'headless app exited before readiness')
                    try:
                        candidate = Client(path, min(0.25, max(0.001, deadline - time.monotonic())))
                    except (FileNotFoundError, ConnectionRefusedError, socket.timeout):
                        time.sleep(0.02)
                        continue
                    client = candidate
                    hello = client.call(2, 'hello', deadline=deadline)
                    require(hello.get('version') == 2 and hello.get('capabilities') == ['block_actions', 'tasks', 'palette'], 'v2 hello')
                    break
                require(client is not None, 'socket readiness exceeded 5 seconds')
                observations = observe(client, transcript)
                if fault == 'copy':
                    observations['copy']['data'] = 'WG5l'
                check(observations, 'socket')
                print('1 socket actions PASS', flush=True)
            finally:
                if client is not None:
                    client.close()
                stop(process)
        child = None
        with tempfile.TemporaryFile(dir=root) as stdout, tempfile.TemporaryFile(dir=root) as stderr:
            try:
                child = subprocess.Popen([str(helper), '--transcript', str(transcript)], stdout=stdout, stderr=stderr)
                child.wait(timeout=30)
                stderr.seek(0)
                require(child.returncode == 0, 'palette helper failed: ' + stderr.read(2000).decode(errors='replace'))
                stdout.seek(0)
                payload = stdout.read(2 * 1024 * 1024 + 1)
                require(len(payload) <= 2 * 1024 * 1024, 'palette observations oversized')
                check(json.loads(payload), 'palette')
                print('2 palette actions PASS', flush=True)
            finally:
                stop(child, helper=True)
    # M2 includes M1. Do not run either a second time.
    previous = None
    try:
        previous = subprocess.Popen([str(Path(__file__).resolve().with_name('gate-m2.sh'))])
        require(previous.wait(timeout=180) == 0, 'M2 regression failed')
    finally:
        stop(previous)


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
