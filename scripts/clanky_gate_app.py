"""Which Clanky the gate harnesses drive, and the environment every one of them must use.

CLANKY_BIN names another window binary (clanky-gpui) to drive instead of today's clanky from the build folder. That
binary is also its own session server (`--server --socket PATH`), as today's clanky is. Features it does not have
yet (tabs, chrome, the startup robot, cursor blinking) are skipped by the gates, which say so in their output.

isolated_env() is the environment for any window or server a gate starts: no variable that reaches the user's
desktop session (a Wayland display, the D-Bus session bus) and nothing that looks like a credential.
"""

import os
from pathlib import Path

DESKTOP = ('WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'XDG_SESSION_TYPE')
CREDENTIAL_WORDS = ('KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD')


def override():
    """The window binary from CLANKY_BIN, or None for today's clanky."""
    value = os.environ.get('CLANKY_BIN')
    if not value:
        return None
    path = Path(value)
    if not (path.is_file() and os.access(path, os.X_OK)):
        raise RuntimeError(f'CLANKY_BIN={value} is not an executable file')
    return path


def window_binary(bins):
    return override() or Path(bins) / 'clanky'


def server_argv(bins, socket):
    return [str(window_binary(bins)), '--server', '--socket', str(socket)]


def agent_gate(bins):
    """The owner test client: CLANKY_AGENT_GATE when set, else the one in the build folder."""
    return Path(os.environ.get('CLANKY_AGENT_GATE') or Path(bins) / 'clanky-agent-gate')


def isolated_env(base=None):
    env = dict(os.environ if base is None else base)
    for key in list(env):
        if key in DESKTOP or any(word in key.upper() for word in CREDENTIAL_WORDS):
            env.pop(key)
    return env


def single_pane():
    """True when the window under test has one pane and no chrome (clanky-gpui in its first milestone)."""
    return override() is not None


# Error output of the windows and servers a gate starts, kept in its scratch folder instead of /dev/null, so a window
# that exits early names its cause.
WINDOW_LOGS = []


def window_log(path):
    """An open file for a started window's or server's error output; print_window_errors shows its end."""
    WINDOW_LOGS.append(Path(path))
    return open(path, 'ab')  # closed by the caller with the process, or by the interpreter at exit


def print_window_errors(lines=20):
    """Prints the last LINES lines of every kept error output that has any, as note lines, before a gate's FAIL."""
    for path in WINDOW_LOGS:
        if path.exists() and path.stat().st_size:
            for line in path.read_text(errors='replace').splitlines()[-lines:]:
                print(f'note: window error ({path.parent.name}/{path.name}): {line}', flush=True)
