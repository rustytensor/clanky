#!/usr/bin/env python3
"""Private-bus and fresh-server acceptance inputs for session polish."""
import json
import os
import pathlib
import select
import signal
import subprocess
import sys
import tempfile
import time

helper, clanky = (str(pathlib.Path(p).resolve()) for p in sys.argv[1:])
build = pathlib.Path('/srv/devops/build/clanky-pkg')
build.mkdir(parents=True, exist_ok=True)
receiver_code = '''\
import json, os
import dbus, dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib
DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
name = dbus.service.BusName('org.freedesktop.Notifications', bus)
class Receiver(dbus.service.Object):
    @dbus.service.method('org.freedesktop.Notifications', in_signature='susssasa{sv}i', out_signature='u')
    def Notify(self, app, replace, icon, summary, body, actions, hints, timeout):
        with open(os.environ['CAPTURE'], 'a', encoding='utf-8') as f:
            f.write(json.dumps([str(app), str(summary), str(body)]) + '\\n')
        return 1
receiver = Receiver(bus, '/org/freedesktop/Notifications')
open(os.environ['READY'], 'w').close()
GLib.MainLoop().run()
'''
with tempfile.TemporaryDirectory(prefix='polish-gate-', dir=build) as directory:
    root = pathlib.Path(directory)
    socket = root / 'bus'
    daemon = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--print-address=1',
                               '--address=unix:path=' + str(socket)],
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    receiver = None
    try:
        ready, _, _ = select.select([daemon.stdout], [], [], 10)
        if not ready:
            raise RuntimeError('private D-Bus did not start')
        address = daemon.stdout.readline().strip()
        if not address.startswith('unix:'):
            raise RuntimeError('invalid private D-Bus address')
        capture, marker = root / 'notifications.jsonl', root / 'receiver-ready'
        env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, CAPTURE=str(capture), READY=str(marker))
        env.pop('CLANKY_GATE_FAULT', None)
        receiver = subprocess.Popen([sys.executable, '-c', receiver_code], env=env,
                                    stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        deadline = time.monotonic() + 10
        while not marker.exists():
            if receiver.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError('fake notification receiver did not start')
            time.sleep(0.02)
        subprocess.run([helper, 'notify'], env=env, check=True, timeout=45)
        time.sleep(0.3)
        observed = [json.loads(line) for line in capture.read_text().splitlines()] if capture.exists() else []
        if os.environ.get('CLANKY_GATE_FAULT') == 'polish' and observed:
            observed[0][2] = 'Command finished (exit 8)'
        expected = [['Clanky', 'Clanky', 'Command finished (exit 7)']]
        if observed != expected:
            raise RuntimeError(f'notification: expected {expected!r}, observed {observed!r}')
        first, second = root / 'first', root / 'second'
        first.mkdir()
        second.mkdir()
        layout = root / 'layout.json'
        fixture = {'version': 1, 'active': 0, 'groups': [{'layout': {'Split': {'axis': 'Columns',
            'first': {'Leaf': 'old1'}, 'second': {'Leaf': 'old2'}}}, 'focus': 'old2'}],
            'terminals': {'old1': {'cwd': str(first), 'rows': 24, 'cols': 40},
                          'old2': {'cwd': str(second), 'rows': 30, 'cols': 60}}}
        layout.write_text(json.dumps(fixture))
        layout.chmod(0o600)
        result = subprocess.run([helper, 'restore', str(layout), str(root), clanky], env=env,
                                check=True, timeout=45, text=True, capture_output=True)
        observed = json.loads(result.stdout)
        expected = {'active': 0, 'axis': 'Columns', 'focus': 1,
                    'terminals': [{'cwd': str(first), 'rows': 24, 'cols': 40},
                                  {'cwd': str(second), 'rows': 30, 'cols': 60}],
                    'fresh_shells': True, 'same_server_no_duplicate': True, 'old_command_ran': False}
        if observed != expected:
            raise RuntimeError(f'restore: expected {expected!r}, observed {observed!r}')
    except (RuntimeError, subprocess.SubprocessError, OSError, ValueError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        sys.exit(1)
    finally:
        for proc in (receiver, daemon):
            if proc is not None and proc.poll() is None:
                proc.send_signal(signal.SIGTERM)
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
