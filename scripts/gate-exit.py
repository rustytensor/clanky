#!/usr/bin/env python3
"""Exit gate: a pane closes when its shell exits, as in Ptyxis (Marc, 2026-11-13: "when I close the terminal with
exit the screen stays like that"; his Ptyxis profile has exit-action 'close', which is also Ptyxis's default).

Usage: gate-exit.py [--fault subshell|survivor]
A fault changes the run's input on purpose; the gate must then FAIL.

Real Clanky window on a private Xvfb display (the T2 harness). The window starts with one tab of two panes.
Expected results, fixed before the work (Ptyxis closes the tab whose shell exits; Clanky's own Ctrl+Shift+W already
closes a pane, its tab when it was the last pane, and the window and server after the last terminal):
  X1 `exit` in the focused pane of two: within 3 s that pane's tab_id is gone from list_tabs, the other pane stays,
     and the window keeps running.
  X2 a new tab (Ctrl+Shift+T), then `kill -9 $$` in its only pane (a shell killed by a signal): within 3 s its
     tab_id is gone and the window keeps running with the first tab's remaining pane.
  X3 `exit` in the last pane: within 5 s the window process exits with status 0 and the window's server exits.
"""
import importlib.util, os, sys, time
from pathlib import Path

ROOT = Path(os.environ.get('CLANKY_EXIT_WORK', '/srv/devops/build/clanky-exit-gate-run'))
os.environ['CLANKY_T2_WORK'] = str(ROOT)
spec = importlib.util.spec_from_file_location('t2', Path(__file__).with_name('gate-t2-terminal.py'))
t2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t2)
Fail, require, until, Window = t2.Fail, t2.require, t2.until, t2.Window
FAULT = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == '--fault' else None


def tabs(win):
    return [t['tab_id'] for t in win.o.call(1, 'list_tabs')['tabs']]


def focused(win):
    """The tab a typed marker lands in."""
    marker = f'focus-{time.monotonic_ns()}'
    win.type(f'echo {marker}\n')
    for tab in tabs(win):
        s = win.o.call(4, 'screen', tab_id=tab, history_offset=0, history_limit=64)
        text = '\n'.join(l if isinstance(l, str) else l.get('text', '') for l in s.get('lines', []))
        if text.count(marker) >= 2:
            return tab
    raise Fail('focused pane found')


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def main():
    import shutil
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)
    win = Window('exit', cursor_blink='off')
    try:
        window = win.procs[1]
        first = tabs(win)
        require(len(first) == 2, f'setup: the window starts with two panes: {first}')
        gone = focused(win)
        if FAULT == 'subshell':
            win.type('sh\n')
            time.sleep(.5)
        win.type('exit\n')
        until(lambda: gone not in tabs(win), f'X1 the exited pane {gone} closes', 3)
        require(len(tabs(win)) == 1 and window.poll() is None, f'X1 the other pane and the window stay: {tabs(win)}')
        survivor = tabs(win)[0]
        win.key('ctrl+shift+t')
        until(lambda: len(tabs(win)) == 2, 'X2 a new tab opens', 5)
        time.sleep(1)
        new = next(t for t in tabs(win) if t != survivor)
        win.type('kill -9 $$\n')
        until(lambda: new not in tabs(win), f'X2 the killed shell\'s pane {new} closes', 3)
        require(tabs(win) == [survivor] and window.poll() is None, f'X2 the first tab and the window stay: {tabs(win)}')
        server = win.server
        if FAULT == 'survivor':
            win.type('sh\n')
            time.sleep(.5)
        win.type('exit\n')
        until(lambda: window.poll() is not None, 'X3 the window closes after the last pane exits', 5)
        require(window.returncode == 0, f'X3 the window exits with status 0: {window.returncode}')
        until(lambda: not alive(server), 'X3 the window\'s server exits', 5)
    finally:
        win.close()
    print('PASS: exit')


if __name__ == '__main__':
    try:
        main()
    except Fail as e:
        print(f'FAIL: {e}')
        sys.exit(1)
