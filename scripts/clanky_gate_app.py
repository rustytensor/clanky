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


def abstract_x_displays():
    """Display numbers whose X server listens on the abstract socket @/tmp/.X11-unix/XN. A server started in another
    /tmp (a sandboxed session) has no lock or socket file in ours, but X clients connect to the abstract socket
    first, so a second server on that number would get its clients and they would fail to connect."""
    found = set()
    for line in Path('/proc/net/unix').read_text().splitlines()[1:]:
        name = line.split()[-1]
        if name.startswith('@/tmp/.X11-unix/X') and name[17:].isdigit():
            found.add(int(name[17:]))
    return found


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


def gpui_cell(family, size_pt, height_scale, scale=1.0):
    """clanky-gpui's terminal cell in device px for a config's font, worked out from the font file (fc-match), never
    from the binary: the width is the advance of 'm', not rounded (clanky-gpui places columns at its multiples), and
    the height is the line height round(size x height_scale x scale), the size being points x 4/3 (the config's
    font_size_pt rule)."""
    import subprocess
    from fontTools.ttLib import TTFont
    path = subprocess.run(['fc-match', '-f', '%{file}', family], capture_output=True, text=True, check=True).stdout
    font = TTFont(path)
    size = size_pt * 4 / 3
    advance = font['hmtx'][font.getBestCmap()[ord('m')]][0] / font['head'].unitsPerEm
    return advance * size * scale, round(size * height_scale * scale)


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
