#!/usr/bin/env python3
"""M1 acceptance on real processes and pixels; expectations live here."""
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
        self.next_id = 0
        self.pending = bytearray()

    def close(self):
        self.sock.close()

    def call(self, op, deadline=None, **args):
        self.next_id += 1
        request_id = str(self.next_id)
        deadline = deadline or time.monotonic() + 10
        self.sock.settimeout(max(0.001, deadline - time.monotonic()))
        wire = dict(v=1, id=request_id, op=op, **args)
        self.sock.sendall(json.dumps(wire).encode('utf-8') + b'\n')
        while b'\n' not in self.pending:
            require(time.monotonic() < deadline, op + ': response deadline')
            self.sock.settimeout(max(0.001, deadline - time.monotonic()))
            data = self.sock.recv(65536)
            require(data, op + ': disconnected')
            self.pending.extend(data)
            require(len(self.pending) <= 2 * 1024 * 1024, op + ': oversized response')
        line, _, remaining = self.pending.partition(b'\n')
        self.pending = bytearray(remaining)
        reply = json.loads(line)
        require(reply.get('id') == request_id, op + ': wrong response id')
        require(reply.get('ok') is True, op + ': ' + repr(reply.get('error')))
        require('result' in reply, op + ': missing result')
        return reply['result']


def open_tab(client):
    return client.call('open_tab', rows=24, cols=80, allow_agent=True)['tab_id']


def run(client, tab, script, cwd, deadline):
    return client.call(
        "run", deadline=deadline, tab_id=tab, argv=["sh", "-c", script], cwd=str(cwd)
    )["command_id"]


def wait(client, tab, command, deadline):
    while time.monotonic() < deadline:
        status = client.call('wait', deadline=deadline, tab_id=tab,
                             command_id=command, timeout_ms=1000)
        if status.get('done') is True:
            require(status.get('state') == 'exited', 'completed command state must be exited')
            return status
    raise AssertionError('PTY completion exceeded 10 seconds')


def output(client, tab, command, expected, deadline):
    result = client.call(
        "get_output",
        deadline=deadline,
        tab_id=tab,
        command_id=command,
        offset=0,
        limit=65536,
        encoding="base64",
    )
    require(result.get('encoding') == 'base64', 'raw output encoding')
    raw = base64.b64decode(result['data'], validate=True)
    require(raw == expected, 'raw output expected %r / observed %r' % (expected, raw))
    require(result.get('truncated') is False, 'raw output truncated')
    require(result.get('eof') is True, 'raw output not fully drained')
    require(result.get('retained_start') == 0, 'raw output evicted')
    require(result.get('next_offset') == len(expected), 'raw next_offset mismatch')
    require(result.get('total_bytes') == len(expected), 'raw total_bytes mismatch')


def pixels(binary):
    rendered = subprocess.run([str(binary), '--case', 'text'], capture_output=True,
                              text=True, timeout=30, check=False)
    require(rendered.returncode == 0, 'text renderer failed (adapter/render/readback): ' + rendered.stderr[-2000:])
    samples = json.loads(rendered.stdout)
    for name, expected in [('red', (255, 0, 0)), ('black', (0, 0, 0))]:
        observed = samples.get(name)
        require(isinstance(observed, list) and len(observed) == 4,
                name + ': expected RGBA sample')
        require(all(type(v) is int and 0 <= v <= 255 for v in observed),
                name + ': invalid RGBA channel')
        require(observed[3] == 255, name + ': expected alpha 255 / observed ' + repr(observed[3]))
        require(all(abs(observed[i] - expected[i]) <= 8 for i in range(3)),
                name + ': expected RGB %r +/-8 / observed %r' % (expected, observed[:3]))


def gate():
    fault = os.environ.get('CLANKY_GATE_FAULT', '')
    require(fault in ('', 'exitcode'), 'unknown CLANKY_GATE_FAULT: ' + fault)
    target = Path(os.environ.get('CARGO_TARGET_DIR', '/srv/devops/build/clanky')).resolve()
    app = target / 'debug' / 'clanky'
    renderer = target / 'debug' / 'clanky-text-gate'
    for binary in (app, renderer):
        require(binary.is_file() and os.access(binary, os.X_OK), 'missing executable: ' + str(binary))
    with tempfile.TemporaryDirectory(prefix='clanky-m1-') as directory:
        root = Path(directory)
        os.chmod(root, 0o700)
        path = root / 'agent.sock'
        client = None
        process = None
        with tempfile.TemporaryFile(mode='w+b') as log:
            try:
                process = subprocess.Popen([str(app), '--headless', '--socket', str(path)],
                                           stdout=log, stderr=log)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    require(process.poll() is None, 'headless app exited before readiness')
                    try:
                        candidate = Client(path, min(0.25, max(0.001, deadline - time.monotonic())))
                    except (FileNotFoundError, ConnectionRefusedError, socket.timeout):
                        time.sleep(0.02)
                        continue
                    try:
                        hello = candidate.call('hello', deadline=deadline)
                        require(hello.get('version') == 1, 'hello version must be 1')
                        require(hello.get('capabilities') == ['tabs', 'commands', 'blocks'], 'hello capabilities')
                    except BaseException:
                        candidate.close()
                        raise
                    client = candidate
                    break
                require(client is not None, 'socket hello readiness exceeded 5 seconds')

                tab = open_tab(client)
                deadline = time.monotonic() + 10
                command = run(client, tab, "printf hello; exit 3", root, deadline)
                status = wait(client, tab, command, deadline)
                observed = status.get('exit_code')
                if fault == 'exitcode':
                    observed = 0
                require(observed == 3, 'exit code expected 3 / observed %r' % observed)
                require(status.get('signal') is None, 'hello command unexpectedly signaled')
                output(client, tab, command, b"hello", deadline)
                print('1 socket/process PASS', flush=True)

                script = ("printf '\\033]133;A\\007\\033]133;B\\007\\033]133;C\\007'\n"
                          "echo one\n"
                          "printf '\\033]133;D;0\\007\\033]133;A\\007\\033]133;B\\007\\033]133;C\\007'\n"
                          "echo two\n"
                          "printf '\\033]133;D;0\\007'\n")
                deadline = time.monotonic() + 10
                command = run(client, tab, script, root, deadline)
                status = wait(client, tab, command, deadline)
                require(status.get('exit_code') == 0 and status.get('signal') is None, 'OSC shell status')
                blocks = client.call(
                    "list_blocks",
                    deadline=deadline,
                    tab_id=tab,
                    command_id=command,
                    after=None,
                    limit=64,
                )
                require(blocks.get('evicted') == 0 and blocks.get('has_more') is False, 'OSC blocks evicted/paginated')
                items = blocks.get('blocks')
                require(isinstance(items, list) and len(items) == 2, 'expected exactly two OSC blocks')
                for block, text in zip(items, ('one', 'two')):
                    require(block.get('output') == text, 'OSC output expected %r / observed %r' % (text, block.get('output')))
                    require(block.get('complete') is True and block.get('truncated') is False, 'OSC block incomplete/truncated')
                    require(block.get('exit_code') == 0, 'OSC block exit code must be 0')
                    require(block.get('command_id') == command, 'OSC block belongs to wrong command')
                print('2 OSC blocks PASS', flush=True)

                pixels(renderer)
                print('3 text pixels PASS', flush=True)

                first, second = open_tab(client), open_tab(client)
                require(first != second, 'tabs must have distinct IDs')
                deadline = time.monotonic() + 10
                a = run(client, first, "printf A", root, deadline)
                b = run(client, second, "printf B", root, deadline)
                for tab_id, command_id, expected in ((first, a, b'A'), (second, b, b'B')):
                    status = wait(client, tab_id, command_id, deadline)
                    require(status.get('exit_code') == 0 and status.get('signal') is None, 'tab command status')
                    output(client, tab_id, command_id, expected, deadline)
                tabs = client.call("list_tabs", deadline=deadline)["tabs"]
                ids = {entry['tab_id'] for entry in tabs}
                require({first, second} <= ids, 'list_tabs must contain both independent tabs')
                print('4 independent tabs PASS', flush=True)
            finally:
                if client is not None:
                    client.close()
                if process is not None:
                    if process.poll() is None:
                        process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
        # TemporaryDirectory removes only this gate's private socket directory.


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
