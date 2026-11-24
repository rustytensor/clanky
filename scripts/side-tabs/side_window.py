"""The side tabs gate's window: the chrome harness's real clanky-gpui window, with the fixtures the side column needs.

The window is chrome_harness.RealWindow (a private Xvfb, an isolated HOME and XDG folders, the control record, the
frame trace, xdotool input), started without the harness's second tab and with extra environment variables such as
CLANKY_REQUEST_LOG. Its HOME holds the fixtures, made fresh for each window:

  ~/alpha            a git repository on branch side-gate, with a folder ~/alpha/src
  ~/beta             a git repository on branch main
  ~/plain-folder     a folder outside any repository
  ~/<long parent>/<untrusted name>
                     a folder outside any repository whose name holds ESC, BEL and U+202E; its path is in
                     ~/weird-path.txt so a shell can reach it without typing those characters
  ~/bin/pi           a fake agent program: a link to cat, so its process is named "pi" and it reads stdin

Everything the gate types reaches a pane through the owner API's input operation, so no keystroke depends on which
part of the window has the keyboard.
"""

import base64
import ctypes
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "window-chrome"))
import chrome_harness
from chrome_harness import Conn, Fail, RealWindow, free_display, until

SCREEN = "main-window"
# The untrusted folder name: each character the window must never draw or record raw, once, inside a name long
# enough to need an ellipsis. A Linux file name holds at most 255 bytes (U+202E takes 3), so the long parent folder
# makes the whole path longer than 300 characters.
UNTRUSTED_NAME = (
    "evil"
    + chr(0x1B)
    + "[31m-bell"
    + chr(0x07)
    + "-flip"
    + chr(0x202E)
    + "-"
    + "x" * 220
)
LONG_PARENT = "p" * 90
GIT = shutil.which("git") or "git"


def git(folder, *args):
    """Runs git in `folder` with no user or system configuration, so the fixtures are the same on every machine."""
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": str(folder),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_AUTHOR_NAME": "gate",
        "GIT_AUTHOR_EMAIL": "gate@example.invalid",
        "GIT_COMMITTER_NAME": "gate",
        "GIT_COMMITTER_EMAIL": "gate@example.invalid",
    }
    subprocess.run(
        [GIT, *args], cwd=folder, env=env, check=True, stdout=subprocess.DEVNULL
    )


def make_repo(folder, branch):
    folder.mkdir(parents=True, exist_ok=True)
    git(folder, "init", "-q", "-b", branch)
    git(folder, "commit", "-q", "--allow-empty", "-m", "fixture")


def make_fixtures(home):
    """The folders and programs of the module docstring, under `home`; returns the untrusted folder's path."""
    make_repo(home / "alpha", "side-gate")
    (home / "alpha/src").mkdir(exist_ok=True)
    make_repo(home / "beta", "main")
    (home / "plain-folder").mkdir(exist_ok=True)
    weird = home / LONG_PARENT / UNTRUSTED_NAME
    weird.mkdir(parents=True, exist_ok=True)
    (home / "weird-path.txt").write_text(str(weird), encoding="utf-8")
    bin_dir = home / "bin"
    bin_dir.mkdir(exist_ok=True)
    pi = bin_dir / "pi"
    if not pi.exists():
        pi.symlink_to(shutil.which("cat") or "/usr/bin/cat")
    return weird


class SideWindow(RealWindow):
    """One clanky-gpui window at scale 1 with the fixtures; `extra_env` is added to the window's environment."""

    def __init__(self, work, app, config, fault=None, extra_env=None, keep=False):
        self.extra_env = dict(extra_env or {})
        super().__init__(1, work, app, config, fault, keep)

    def _start(self):
        home = self.work / "home"
        self.weird = make_fixtures(home)
        with (home / ".bashrc").open("a", encoding="utf-8") as rc:
            rc.write('export PATH="$HOME/bin:$PATH"\n')
        env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith(
                (
                    "XDG_",
                    "CLANKY",
                    "WAYLAND",
                    "DBUS",
                    "GNOME",
                    "GSETTINGS",
                    "VTE",
                    "DISPLAY",
                    "GIT_",
                )
            )
        }
        env.update(
            HOME=str(home),
            XDG_CONFIG_HOME=str(self.work / "config"),
            XDG_DATA_HOME=str(self.work / "data"),
            XDG_STATE_HOME=str(self.work / "state"),
            XDG_CACHE_HOME=str(self.work / "cache"),
            XDG_RUNTIME_DIR=str(self.work / "runtime"),
            SHELL="/bin/bash",
            TZ="UTC",
            GSETTINGS_BACKEND="memory",
            GPUI_X11_SCALE_FACTOR="1",
            LP_NUM_THREADS="8",
            VK_DRIVER_FILES=chrome_harness.VULKAN_ICD,
            GIT_CONFIG_NOSYSTEM="1",
            GIT_CONFIG_GLOBAL=os.devnull,
            CLANKY_CONTROL_RECORD=str(self.record_path),
            CLANKY_FRAME_TRACE=str(self.work / "frames.csv"),
        )
        env.update(self.extra_env)
        if self.fault:
            env["CLANKY_CONTROL_FAULT"] = self.fault
        config = self.work / "config.json"
        config.write_text(json.dumps(self.config), encoding="utf-8")
        self.config_path = config
        read_end, write_end = os.pipe()
        xvfb = subprocess.Popen(
            [
                "Xvfb",
                ":" + free_display(),
                "-displayfd",
                str(write_end),
                "-screen",
                "0",
                f"{self.pw + 200}x{self.ph + 200}x24",
                "-nolisten",
                "tcp",
            ],
            pass_fds=(write_end,),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.procs.append(xvfb)
        os.close(write_end)
        env["DISPLAY"] = ":" + os.read(read_end, 32).decode().strip()
        os.close(read_end)
        self.env = env
        sock = self.work / "window.sock"
        self.error_file = self.work / "window.err"
        window = subprocess.Popen(
            [
                str(self.app),
                "--socket",
                str(sock),
                "--config",
                str(config),
                "--size",
                f"{chrome_harness.LOGICAL_W}x{chrome_harness.LOGICAL_H}",
            ],
            env=env,
            cwd=str(home),
            stdout=subprocess.DEVNULL,
            stderr=self.error_file.open("wb"),
        )
        self.procs.append(window)
        self.process = window

        def socket_ready():
            if window.poll() is not None:
                raise Fail(f"window exited: {self.errors()[-300:]}")
            return sock.exists()

        until(socket_ready, "window socket", 40)
        self.wid = until(
            lambda: (
                subprocess.run(
                    ["xdotool", "search", "--pid", str(window.pid)],
                    check=False,
                    env=env,
                    capture_output=True,
                    text=True,
                ).stdout.split()
                or [None]
            )[0],
            "window",
            40,
        )
        self.x("windowsize", "--sync", self.wid, str(self.pw), str(self.ph))
        self.x("windowmove", "--sync", self.wid, "0", "0")
        self.owner = Conn(sock)
        self.owner.call(3, "auth", token=Path(str(sock) + ".owner").read_text().strip())
        self.server = self.owner.call(3, "hello").get("server_pid")
        until(lambda: self.owner.call(1, "list_tabs")["tabs"], "first pane", 20)
        time.sleep(2)
        self.x("windowfocus", "--sync", self.wid)
        self.away()

    # ------------------------------------------------------------ the window's process and the server

    def errors(self):
        """What the window wrote to stderr, where config errors appear."""
        try:
            return self.error_file.read_text(encoding="utf-8", errors="replace")
        except OSError:
            return ""

    def workspace(self):
        return self.owner.call(3, "workspace_get")

    def pane_of(self, tab):
        """The focused pane of tab `tab` (its place in the tab list)."""
        return self.workspace()["groups"][tab]["focus"]

    def panes(self):
        """Every pane of the workspace."""

        def leaves(layout):
            if "Leaf" in layout:
                return [layout["Leaf"]]
            split = layout["Split"]
            return leaves(split["first"]) + leaves(split["second"])

        return [
            leaf for g in self.workspace()["groups"] for leaf in leaves(g["layout"])
        ]

    def send(self, tab, text):
        """Types `text` into tab `tab`'s focused pane through the owner API."""
        data = base64.b64encode(text.encode()).decode()
        self.owner.call(3, "input", tab_id=self.pane_of(tab), data=data)

    def new_tab(self):
        """Opens a tab with Ctrl+Shift+T and waits for it and for its shell to have run .bashrc; returns its index.

        The window's command input hides the shell's prompt, so the shell is ready once it is in ~/HIH, where
        .bashrc takes it."""
        count = len(self.workspace()["groups"])
        self.x("windowfocus", "--sync", self.wid)
        self.key("ctrl+shift+t")
        until(
            lambda: len(self.workspace()["groups"]) > count,
            f"tab {count + 1} opens",
            20,
        )
        pane = self.pane_of(count)
        until(lambda: self.cwd(pane).endswith("/HIH"), f"tab {count + 1} shell", 20)
        return count

    def cwd(self, pane):
        try:
            return self.owner.call(3, "terminal_cwd", tab_id=pane).get("cwd", "")
        except Fail:
            return ""

    def unfocus(self):
        """Takes the keyboard focus from every window (X focus None), as switching to another program does."""
        x11 = ctypes.CDLL("libX11.so.6")
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        display = x11.XOpenDisplay(self.env["DISPLAY"].encode())
        if not display:
            raise Fail("cannot open the X display to move the focus")
        handle = ctypes.c_void_p(display)
        x11.XSetInputFocus(
            handle, ctypes.c_ulong(0), ctypes.c_int(0), ctypes.c_ulong(0)
        )
        x11.XSync(handle, ctypes.c_int(0))
        x11.XCloseDisplay(handle)

    # ------------------------------------------------------------ the control record

    def controls(self):
        line = self.screen(SCREEN)
        return line.get("controls", []) if line else []

    def control(self, ident):
        return next((c for c in self.controls() if c.get("id") == ident), None)

    def rows(self):
        """The side column's list-row controls now on the screen, in record order."""
        return [
            c
            for c in self.controls()
            if c.get("kind") == "list-row" and c.get("group") == "side-tabs"
        ]

    def landmark_lines(self, name):
        """{index: line}: the last landmark line written for each index of `name`, every field kept."""
        latest = {}
        for line in self.records():
            if line.get("record") == "landmark" and line.get("name") == name:
                latest[line.get("index", 0)] = line
        return latest

    def landmark_box(self, name, index=0):
        line = self.landmark_lines(name).get(index)
        return line.get("box") if line else None

    def rows_by_tab(self):
        """{tab index: line}: the last side-tab-row landmark written for each tab."""
        latest = {}
        for line in self.records():
            if line.get("record") == "landmark" and line.get("name") == "side-tab-row":
                latest[line.get("tab")] = line
        return latest

    def record_frames(self):
        return sum(1 for line in self.records() if line.get("record") == "frame")

    def click_box(self, box):
        x, y, w, h = box
        self.move(x + w / 2, y + h / 2)
        time.sleep(0.15)
        self.x("click", "1")
        time.sleep(0.3)

    def wheel(self, x, y, notches, down=True):
        self.move(x, y)
        time.sleep(0.15)
        for _ in range(notches):
            self.x("click", "5" if down else "4")
            time.sleep(0.08)
