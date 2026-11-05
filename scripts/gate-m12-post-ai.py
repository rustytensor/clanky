#!/usr/bin/env python3
"""M12 Post-AI checks against a real private server and owner connection."""
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import sys
import tempfile
import time


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run():
    target = Path(os.environ['CARGO_TARGET_DIR'])
    app = target / 'debug/clanky'
    helper = target / 'debug/clanky-agent-gate'
    with tempfile.TemporaryDirectory(prefix='m12-post-ai-', dir=target) as temp:
        path = Path(temp) / 'server.sock'
        server = subprocess.Popen([app, '--server', '--post-ai', '--socket', path],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        owner = None
        try:
            deadline = time.monotonic() + 10
            while not path.exists():
                require(server.poll() is None and time.monotonic() < deadline, 'server startup')
                time.sleep(.02)
            with socket.socket(socket.AF_UNIX) as agent:
                agent.settimeout(5)
                agent.connect(str(path))
                agent.sendall(b'{"v":3,"id":"1","op":"hello"}\n')
                with agent.makefile('rb') as stream:
                    response = json.loads(stream.readline(65537))
                require(response.get('ok') is False and response.get('error', {}).get('message') ==
                        'agents are off (Post-AI mode)', 'agent hello refusal')
            owner = subprocess.Popen([helper, 'owner', path], stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            owner.stdin.write(b'{"v":3,"op":"hello","args":{}}\n')
            owner.stdin.flush()
            require(bool(select.select([owner.stdout], [], [], 5)[0]), 'owner hello deadline')
            result = json.loads(owner.stdout.readline(65537))
            require(result.get('ok') is True, 'owner hello')
            mcp = subprocess.run([app, 'mcp', '--socket', path], input=b'', capture_output=True, timeout=10)
            require(mcp.returncode == 2 and b'agents are off (Post-AI mode)' in mcp.stderr,
                    'MCP exit and refusal message')
            print('PASS: Post-AI agent hello refused, owner hello accepted, MCP exit 2')
        finally:
            if owner:
                owner.terminate()
                try: owner.wait(timeout=3)
                except subprocess.TimeoutExpired: owner.kill(); owner.wait(timeout=3)
            server.terminate()
            try: server.wait(timeout=5)
            except subprocess.TimeoutExpired: server.kill(); server.wait(timeout=3)


if __name__ == '__main__':
    try: run()
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f'FAIL: M12 Post-AI: {error}', file=sys.stderr)
        sys.exit(1)
